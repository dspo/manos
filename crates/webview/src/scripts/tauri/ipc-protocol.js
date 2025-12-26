// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

;(function () {
  /**
   * A runtime generated key to ensure an IPC call comes from an initialized frame.
   *
   * This is declared outside the `window.__TAURI_INVOKE__` definition to prevent
   * the key from being leaked by `window.__TAURI_INVOKE__.toString()`.
   */
  const __TAURI_INVOKE_KEY__ = __TEMPLATE_invoke_key__

  const processIpcMessage = __RAW_process_ipc_message_fn__
  const osName = __TEMPLATE_os_name__
  const fetchChannelDataCommand = __TEMPLATE_fetch_channel_data_command__
  let customProtocolIpcFailed = false

  // on Android we never use it because Android does not have support to reading the request body
  const canUseCustomProtocol = osName !== 'android'

  function sendIpcMessage(message) {
    const { cmd, callback, error, payload, options } = message

    console.log("customProtocolIpcFailed: ", customProtocolIpcFailed)
    console.log("canUseCustomProtocol: ", canUseCustomProtocol)
    console.log("fetchChannelDataCommand: ", fetchChannelDataCommand, ", cmd: ", cmd)

    if (
      !customProtocolIpcFailed
      && (canUseCustomProtocol || cmd === fetchChannelDataCommand)
    ) {
      const { contentType, data } = processIpcMessage(payload)

      const headers = new Headers((options && options.headers) || {})
      headers.set('Content-Type', contentType)
      headers.set('Tauri-Callback', callback)
      headers.set('Tauri-Error', error)
      headers.set('Tauri-Invoke-Key', __TAURI_INVOKE_KEY__)

      fetch(window.__TAURI_INTERNALS__.convertFileSrc(cmd, 'ipc'), {
        method: 'POST',
        body: data,
        headers
      })
        .then((response) => {
            console.log("response.url: ", response.url)
            response.headers.forEach((value, key) => {
                console.log(`response header: ${key}: ${value}`);
            });
            console.log("response status: ", response.headers.get('Tauri-Response'))
            console.log("response access-control-allow-origin: ", response.headers.get('access-control-allow-origin'))

          const callbackId =
            response.headers.get('Tauri-Response') === 'ok' ? callback : error
          // we need to split here because on Android the content-type gets duplicated
          switch ((response.headers.get('content-type') || '').split(',')[0]) {
            case 'application/json':
              console.log(`response application/json, callbackId: ${callbackId}`)
              return response.json().then((r) => {
                  console.log("response json: ", r)
                  return [callbackId, r]
              })
            case 'text/plain':
              console.log("response text/plain")
              return response.text().then((r) => {
                  console.log("response text: ", r)
                  return [callbackId, r]
              })
            default:
              return response.arrayBuffer().then((r) => {
                  console.log("response arrayBuffer: ", r)
                  return [callbackId, r]
              })
          }
        })
        .catch((e) => {
          console.warn(
            'IPC custom protocol failed, Tauri will now use the postMessage interface instead',
            e
          )
          // failed to use the custom protocol IPC (either the webview blocked a custom protocol or it was a CSP error)
          // so we need to fallback to the postMessage interface
          customProtocolIpcFailed = true
          sendIpcMessage(message)
        })
        .then(([callbackId, data]) => {
          console.log("response data: ", data)
          window.__TAURI_INTERNALS__.runCallback(callbackId, data)
        })
    } else {
      // otherwise use the postMessage interface
      const { data } = processIpcMessage({
        cmd,
        callback,
        error,
        options: {
          ...options,
          customProtocolIpcBlocked: customProtocolIpcFailed
        },
        payload,
        __TAURI_INVOKE_KEY__,
        webviewLabel:
          (window.__TAURI_INTERNALS__
            && window.__TAURI_INTERNALS__.metadata
            && window.__TAURI_INTERNALS__.metadata.currentWebview
            && window.__TAURI_INTERNALS__.metadata.currentWebview.label)
          || null
      })
      window.ipc.postMessage(data)
    }
  }

  Object.defineProperty(window.__TAURI_INTERNALS__, 'postMessage', {
    value: sendIpcMessage
  })
})()
