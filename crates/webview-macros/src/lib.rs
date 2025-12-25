//! gpui-manos-webview macros

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Ident};

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
                fn response_builder(status_code: StatusCode) -> http::response::Builder {
                    http::Response::builder()
                        .status(status_code)
                        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"))
                        .header(http::header::ACCESS_CONTROL_EXPOSE_HEADERS, "Tauri-Response")
                        .header("Tauri-Response", "ok")
                }

                fn response_bad_request<S: ToString>(content: S) -> http::Result<http::Response<Vec<u8>>> {
                    response_builder(StatusCode::BAD_REQUEST)
                        .header(CONTENT_TYPE, HeaderValue::from_static("text/plain"))
                        .body(content.to_string().into_bytes())
                }

                fn response_internal_server_error<S: ToString>(content: S) -> http::Result<http::Response<Vec<u8>>> {
                    response_builder(StatusCode::INTERNAL_SERVER_ERROR)
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

                        response_builder(StatusCode::OK)
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
                    fn response_builder(status_code: StatusCode) -> http::response::Builder {
                        http::Response::builder()
                            .status(status_code)
                            .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"))
                            .header(http::header::ACCESS_CONTROL_EXPOSE_HEADERS, "Tauri-Response")
                            .header("Tauri-Response", "ok")
                    }

                    fn response_bad_request<S: ToString>(content: S) -> http::Result<http::Response<Vec<u8>>> {
                        response_builder(StatusCode::BAD_REQUEST)
                            .header(CONTENT_TYPE, HeaderValue::from_static("text/plain"))
                            .body(content.to_string().into_bytes())
                    }

                    fn response_internal_server_error<S: ToString>(content: S) -> http::Result<http::Response<Vec<u8>>> {
                        response_builder(StatusCode::INTERNAL_SERVER_ERROR)
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

                            response_builder(StatusCode::OK)
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
