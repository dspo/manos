//! gpui-manos-webview macros

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    AttributeArgs, FnArg, Ident, ItemFn, Lit, Meta, NestedMeta, Pat, Path, ReturnType, Token, Type,
    parse_macro_input,
};

#[proc_macro_attribute]
pub fn command(attributes: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attributes as AttributeArgs);
    let function = parse_macro_input!(item as ItemFn);

    if function.sig.asyncness.is_some() {
        return syn::Error::new_spanned(
            function.sig.fn_token,
            "async commands are not supported yet",
        )
        .to_compile_error()
        .into();
    }

    let mut root: Path = syn::parse_str("::gpui_manos_webview").expect("valid default root path");
    let mut rename_all = "camelCase".to_string();

    for arg in args {
        let NestedMeta::Meta(meta) = arg else {
            return syn::Error::new_spanned(arg, "unexpected attribute argument")
                .to_compile_error()
                .into();
        };

        match meta {
            Meta::NameValue(nv) if nv.path.is_ident("rename_all") => {
                let Lit::Str(value) = &nv.lit else {
                    return syn::Error::new_spanned(
                        &nv.lit,
                        "expected a string literal (\"camelCase\" or \"snake_case\")",
                    )
                    .to_compile_error()
                    .into();
                };

                let value = value.value();
                match value.as_str() {
                    "camelCase" | "snake_case" => rename_all = value,
                    _ => {
                        return syn::Error::new_spanned(
                            &nv.lit,
                            "expected \"camelCase\" or \"snake_case\"",
                        )
                        .to_compile_error()
                        .into();
                    }
                }
            }
            Meta::NameValue(nv) if nv.path.is_ident("root") => {
                let Lit::Str(value) = &nv.lit else {
                    return syn::Error::new_spanned(&nv.lit, "expected a string literal")
                        .to_compile_error()
                        .into();
                };

                let value = value.value();
                let path = if value == "crate" {
                    "crate".to_string()
                } else {
                    format!("::{value}")
                };

                root = match syn::parse_str::<Path>(&path) {
                    Ok(path) => path,
                    Err(err) => return err.to_compile_error().into(),
                };
            }
            other => {
                return syn::Error::new_spanned(
                    other,
                    "unsupported attribute argument (supported: root = \"...\", rename_all = \"...\")",
                )
                .to_compile_error()
                .into();
            }
        }
    }

    let command_fn = function.sig.ident.clone();
    let wrapper_fn = format_ident!("__cmd__{}", command_fn);
    let args_struct = format_ident!("__gpui_cmd_args__{}", command_fn);
    let vis = &function.vis;

    let mut arg_idents = Vec::new();
    let mut arg_types = Vec::new();
    for input in &function.sig.inputs {
        match input {
            FnArg::Receiver(receiver) => {
                return syn::Error::new_spanned(receiver, "commands must be free functions")
                    .to_compile_error()
                    .into();
            }
            FnArg::Typed(pat_type) => match &*pat_type.pat {
                Pat::Ident(pat_ident) => {
                    arg_idents.push(pat_ident.ident.clone());
                    arg_types.push((*pat_type.ty).clone());
                }
                other => {
                    return syn::Error::new_spanned(
                        other,
                        "unsupported argument pattern (expected an identifier)",
                    )
                    .to_compile_error()
                    .into();
                }
            },
        }
    }

    let serde_rename_all = rename_all;

    let parse_args = if arg_idents.is_empty() {
        quote!()
    } else {
        quote! {
            #[allow(non_camel_case_types)]
            #[derive(#root::serde::Deserialize)]
            #[serde(rename_all = #serde_rename_all)]
            struct #args_struct {
                #( #arg_idents: #arg_types, )*
            }

            let __gpui_args: #args_struct = match #root::serde_json::from_slice(&__gpui_body) {
                Ok(args) => args,
                Err(err) => return #root::ipc::bad_request(err),
            };

            let #args_struct { #( #arg_idents, )* } = __gpui_args;
        }
    };

    let call = if arg_idents.is_empty() {
        quote!(#command_fn())
    } else {
        quote!(#command_fn(#(#arg_idents),*))
    };

    let respond = match &function.sig.output {
        ReturnType::Type(_, ty) if is_result_type(ty) => quote! {
            match #call {
                Ok(output) => #root::ipc::ok_json(&output),
                Err(err) => #root::ipc::internal_error(err.to_string()),
            }
        },
        _ => quote! {
            let output = #call;
            #root::ipc::ok_json(&output)
        },
    };

    let wrapper = quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        #vis fn #wrapper_fn(request: #root::http::Request<Vec<u8>>) -> #root::http::Response<Vec<u8>> {
            use ::std::string::ToString as _;
            let __gpui_body = request.into_body();
            #parse_args
            #respond
        }
    };

    quote! {
        #function
        #wrapper
    }
    .into()
}

#[proc_macro]
pub fn generate_handler(input: TokenStream) -> TokenStream {
    let command_paths: syn::punctuated::Punctuated<Path, Token![,]> =
        parse_macro_input!(input with syn::punctuated::Punctuated::parse_terminated);

    let mut command_idents = Vec::new();
    let mut wrapper_paths = Vec::new();

    for command_path in command_paths {
        let mut wrapper_path = command_path.clone();
        let last = wrapper_path
            .segments
            .last_mut()
            .expect("parsed command path has no segments");

        let command_ident = last.ident.clone();
        last.ident = format_ident!("__cmd__{}", command_ident);

        command_idents.push(command_ident);
        wrapper_paths.push(wrapper_path);
    }

    let arms =
        command_idents
            .iter()
            .zip(wrapper_paths.iter())
            .map(|(command_ident, wrapper_path)| {
                quote! { stringify!(#command_ident) => Some(#wrapper_path(request)), }
            });

    quote! {
        move |invoke| {
            let command = invoke.command;
            let request = invoke.request;
            match command.as_str() {
                #(#arms)*
                _ => None,
            }
        }
    }
    .into()
}

fn is_result_type(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    type_path
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Result")
}

/// Wraps a function with signature `Fn(http::Request<Vec<u8>> -> http::Response<Vec<u8>>)` into a tuple `(func_name, func)`.
///
/// This macro takes a function name as input and generates code that returns a tuple containing:
/// 1. The function name as a string
/// 2. The function pointer with the correct type
///
/// # Example
/// ```
/// fn my_handler(req: http::Request<Vec<u8>>) -> http::Response<Vec<u8>> { todo!() }
///
/// let (handler_name, handler) = api_handler!(my_handler);
/// // handler_name: String = "my_handler"
/// // handler: fn(http::Request<Vec<u8>>) -> http::Response<Vec<u8>>
/// ```
#[proc_macro]
pub fn api_handler(input: TokenStream) -> TokenStream {
    // Parse the input as a function name identifier
    let func_name = parse_macro_input!(input as Ident);
    let func_str = func_name.to_string();

    // Generate code that returns a tuple of (function_name_string, function_pointer)
    let expanded = quote! {
        (
            #func_str.to_string(),
            #func_name as fn(http::Request<Vec<u8>>) -> http::Response<Vec<u8>>,
        )
    };

    expanded.into()
}

/// Batch-wraps multiple synchronous HTTP handlers into `Vec<(String, Handler)>`.
///
/// Compared to `api_handler`:
/// - Accepts several comma-separated functions at once
/// - Returns a vector ready to be registered into a router
///
/// # Example
/// ```
/// fn foo(req: http::Request<Vec<u8>>) -> http::Response<Vec<u8>> { todo!() }
/// fn bar(req: http::Request<Vec<u8>>) -> http::Response<Vec<u8>> { todo!() }
///
/// let handlers = api_handler![foo, bar];
/// // handlers: Vec<(String, fn(http::Request<Vec<u8>>) -> http::Response<Vec<u8>>)>
/// ```
#[proc_macro]
pub fn api_handlers(input: TokenStream) -> TokenStream {
    let func_names: Vec<Ident> = syn::parse_macro_input!(input with syn::punctuated::Punctuated::<Ident, syn::token::Comma>::parse_terminated)
        .into_iter()
        .collect();

    let tuples = func_names.iter().map(|func_name| {
        let func_str = func_name.to_string();
        quote! {
            (
                #func_str.to_string(),
                #func_name as fn(http::Request<Vec<u8>>) -> http::Response<Vec<u8>>,
            )
        }
    });

    let expanded = quote! {
        vec![#(#tuples),*]
    };

    expanded.into()
}

///
/// `command_handler` is a procedural macro designed to automatically wrap a Rust function
/// into an HTTP handler. This allows the function to be invoked via HTTP requests, making it
/// suitable for use in web services or any application requiring HTTP-based communication.
///
/// # Usage
///
/// The macro takes a single argument, which is the name of the function you want to convert into
/// an HTTP handler. The function should accept a type that implements `DeserializeOwned` and return
/// a type that implements `Serialize`, typically using `serde_json` for JSON serialization.
///
/// ## Example
///
/// ```rust
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Deserialize)]
/// struct MyRequest {
///     id: u32,
/// }
///
/// #[derive(Serialize)]
/// struct MyResponse {
///     message: String,
/// }
///
/// fn my_function(req: MyRequest) -> Result<MyResponse, String> {
///     Ok(MyResponse { message: format!("Hello, your ID is {}", req.id) })
/// }
///
/// // Convert `my_function` into an HTTP handler
/// let (handler_name, handler) = command_handler!(my_function);
/// ```
///
/// # Generated Code Overview
///
/// - **Function Name Conversion**: The input function's name is converted to a string, used as part of the generated HTTP handler.
/// - **HTTP Response Building**: The macro includes helper functions for constructing HTTP responses with appropriate status codes and headers, including handling of CORS.
/// - **Request Body Parsing**: The request body is deserialized from JSON. If deserialization fails, a `400 Bad Request` response is returned.
/// - **Error Handling**: Errors during function execution result in a `500 Internal Server Error` response.
/// - **Response Serialization**: Successful results are serialized back to JSON, with the content type set appropriately.
///
/// # Return Value
///
/// The macro returns a tuple containing:
/// 1. A `String` representing the name of the wrapped function.
/// 2. A closure that acts as the HTTP handler, accepting an `http::Request<Vec<u8>>` and returning an `http::Response<Vec<u8>>`.
///
/// # Dependencies
///
/// - `http`: For creating and manipulating HTTP requests and responses.
/// - `serde` and `serde_json`: For (de)serializing data.
///
/// # Notes
///
/// - The macro assumes that the function being wrapped can handle the deserialized request data and return a serializable response.
/// - Proper error handling within the provided function is essential, as all errors are caught and returned as HTTP 500 errors.
/// - The macro sets up CORS headers to allow cross-origin requests, which might need to be adjusted based on the specific requirements.
///
#[proc_macro]
pub fn command_handler(input: TokenStream) -> TokenStream {
    let func_name = parse_macro_input!(input as Ident);
    let func_str = func_name.to_string();

    let expanded = quote! {
        (
            #func_str.to_string(),
            move |request: http::Request<Vec<u8>>| -> http::Response<Vec<u8>> {
                // Define necessary constants and helper functions
                use http::{header::CONTENT_TYPE, HeaderValue, Response, StatusCode};
                use serde::{de::DeserializeOwned, Serialize};
                use serde_json::{from_slice, to_vec, Value};

                // Response builder
                fn response_builder(status_code: StatusCode, tauri_response: &'static str) -> http::response::Builder {
                    http::Response::builder()
                        .status(status_code)
                        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"))
                        .header(http::header::ACCESS_CONTROL_EXPOSE_HEADERS, "Tauri-Response")
                        .header("Tauri-Response", tauri_response)
                }

                fn response_bad_request<S: ToString>(content: S) -> http::Result<http::Response<Vec<u8>>> {
                    response_builder(StatusCode::BAD_REQUEST, "error")
                        .header(CONTENT_TYPE, HeaderValue::from_static("text/plain"))
                        .body(content.to_string().into_bytes())
                }

                fn response_internal_server_error<S: ToString>(content: S) -> http::Result<http::Response<Vec<u8>>> {
                    response_builder(StatusCode::INTERNAL_SERVER_ERROR, "error")
                        .header(CONTENT_TYPE, HeaderValue::from_static("text/plain"))
                        .body(content.to_string().into_bytes())
                }

                // Try to deserialize request body
                let request_body = request.into_body();
                // Directly try to deserialize, let the compiler infer the type automatically
                let d = match from_slice(&request_body) {
                    Ok(d) => d,
                    Err(e) => {
                        return response_bad_request(e).unwrap();
                    }
                };

                // Call custom command
                let r: Result<_, _> = #func_name(d);

                // Build response based on result
                match r {
                    Ok(output) => {
                        // Serialize successful result
                        let serialized = match to_vec(&output) {
                            Ok(bytes) => bytes,
                            Err(err) => {
                                return response_internal_server_error(err).unwrap();
                            }
                        };

                        response_builder(StatusCode::OK, "ok")
                            .header(
                                CONTENT_TYPE,
                                if from_slice::<Value>(&serialized).is_ok() {
                                    HeaderValue::from_static("application/json")
                                } else {
                                    HeaderValue::from_static("text/plain")
                                },
                            )
                            .body(serialized)
                            .unwrap()
                    }
                    Err(e) => response_internal_server_error(e).unwrap(),
                }
            },
        )
    };

    expanded.into()
}

///
/// `command_handlers` is a procedural macro designed to automatically wrap multiple Rust functions
/// into HTTP handlers. This allows the functions to be invoked via HTTP requests, making them
/// suitable for use in web services or any application requiring HTTP-based communication.
///
/// # Usage
///
/// The macro takes multiple function names as arguments, separated by commas. Each function should
/// accept a type that implements `DeserializeOwned` and return a type that implements `Serialize`,
/// typically using `serde_json` for JSON serialization.
///
/// ## Example
///
/// ```rust
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Deserialize)]
/// struct MyRequest {
///     id: u32,
/// }
///
/// #[derive(Serialize)]
/// struct MyResponse {
///     message: String,
/// }
///
/// fn my_function1(req: MyRequest) -> Result<MyResponse, String> {
///     Ok(MyResponse { message: format!("Hello from function 1, your ID is {}", req.id) })
/// }
///
/// fn my_function2(req: MyRequest) -> Result<MyResponse, String> {
///     Ok(MyResponse { message: format!("Hello from function 2, your ID is {}", req.id) })
/// }
///
/// // Convert multiple functions into HTTP handlers
/// fn view(window: &mut gpui::Window, app: &mut App) -> Entity<WebView> {
///     app.new(|cx: &mut Context<WebView>| {
///         let webview = gpui_wry::Builder::new()
///             .with_webview_id(WebViewId::from("greet"))
///             .serve_static(String::from("examples/apps/greet/dist"))
///             .invoke_handler(command_handlers![my_function1, my_function2])
///             .build_as_child(window)
///             .unwrap();
///         WebView::new(webview, window, cx)
///     })
/// }
/// ```
///
/// # Generated Code Overview
///
/// - **Function Name Conversion**: Each input function's name is converted to a string, used as part of the generated HTTP handler.
/// - **HTTP Response Building**: The macro includes helper functions for constructing HTTP responses with appropriate status codes and headers, including handling of CORS.
/// - **Request Body Parsing**: The request body is deserialized from JSON. If deserialization fails, a `400 Bad Request` response is returned.
/// - **Error Handling**: Errors during function execution result in a `500 Internal Server Error` response.
/// - **Response Serialization**: Successful results are serialized back to JSON, with the content type set appropriately.
///
/// # Return Value
///
/// The macro returns a `Vec` containing tuples, where each tuple contains:
/// 1. A `String` representing the name of the wrapped function.
/// 2. A closure that acts as the HTTP handler, accepting an `http::Request<Vec<u8>>` and returning an `http::Response<Vec<u8>>`.
///
/// # Dependencies
///
/// - `http`: For creating and manipulating HTTP requests and responses.
/// - `serde` and `serde_json`: For (de)serializing data.
///
/// # Notes
///
/// - The macro assumes that the functions being wrapped can handle the deserialized request data and return a serializable response.
/// - Proper error handling within the provided functions is essential, as all errors are caught and returned as HTTP 500 errors.
/// - The macro sets up CORS headers to allow cross-origin requests, which might need to be adjusted based on the specific requirements.
///
#[proc_macro]
pub fn command_handlers(input: TokenStream) -> TokenStream {
    let func_names: Vec<Ident> = syn::parse_macro_input!(input with syn::punctuated::Punctuated::<Ident, syn::token::Comma>::parse_terminated)
        .into_iter()
        .collect();

    let tuples = func_names.iter().map(|func_name| {
        let func_str = func_name.to_string();
        quote! {
            (
                #func_str.to_string(),
                Box::new(move |request: http::Request<Vec<u8>>| -> http::Response<Vec<u8>> {
                    // Define necessary constants and helper functions
                    use http::{header::CONTENT_TYPE, HeaderValue, Response, StatusCode};
                    use serde::{de::DeserializeOwned, Serialize};
                    use serde_json::{from_slice, to_vec, Value};
                    use std::string::ToString;

                    // Response builder
                    fn response_builder(status_code: StatusCode, tauri_response: &'static str) -> http::response::Builder {
                        http::Response::builder()
                            .status(status_code)
                            .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"))
                            .header(http::header::ACCESS_CONTROL_EXPOSE_HEADERS, "Tauri-Response")
                            .header("Tauri-Response", tauri_response)
                    }

                    fn response_bad_request<S: ToString>(content: S) -> http::Result<http::Response<Vec<u8>>> {
                        response_builder(StatusCode::BAD_REQUEST, "error")
                            .header(CONTENT_TYPE, HeaderValue::from_static("text/plain"))
                            .body(content.to_string().into_bytes())
                    }

                    fn response_internal_server_error<S: ToString>(content: S) -> http::Result<http::Response<Vec<u8>>> {
                        response_builder(StatusCode::INTERNAL_SERVER_ERROR, "error")
                            .header(CONTENT_TYPE, HeaderValue::from_static("text/plain"))
                            .body(content.to_string().into_bytes())
                    }

                    // Try to deserialize request body
                    let request_body = request.into_body();
                    // Directly try to deserialize, let the compiler infer the type automatically
                    let d = match from_slice(&request_body) {
                        Ok(d) => d,
                        Err(e) => {
                            return response_bad_request(e).unwrap();
                        }
                    };

                    // Call custom command
                    let r: Result<_, _> = #func_name(d);

                    // Build response based on result
                    match r {
                        Ok(output) => {
                            // Serialize successful result
                            let serialized = match to_vec(&output) {
                                Ok(bytes) => bytes,
                                Err(err) => {
                                    return response_internal_server_error(err).unwrap();
                                }
                            };

                            response_builder(StatusCode::OK, "ok")
                                .header(
                                    CONTENT_TYPE,
                                    if from_slice::<Value>(&serialized).is_ok() {
                                        HeaderValue::from_static("application/json")
                                    } else {
                                        HeaderValue::from_static("text/plain")
                                    },
                                )
                                .body(serialized)
                                .unwrap()
                        }
                        Err(e) => response_internal_server_error(e).unwrap(),
                    }
                }) as Box<dyn Fn(http::Request<Vec<u8>>) -> http::Response<Vec<u8>> + Send + Sync + 'static>,
            )
        }
    });

    let expanded = quote! {
        vec![#(#tuples),*]
    };

    expanded.into()
}
