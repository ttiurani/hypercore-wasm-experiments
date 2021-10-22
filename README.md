# Hypercore WASM experiments with Rust v2.0

This is a followup experiment to [Frando's hypercore WASM experiments](https://github.com/Frando/hypercore-wasm-experiments) to compile the Rust implementation of hypercore-protocol and hypercore to WASM for use in browsers.

- *Update 2021-10-22*: Added WASM hypercore as the storage

What this does (in Rust compiled to WASM):

- Fetch a key, encoded as hex string, from `/key`
- Open a Websocket to localhost:9000
- Open a hypercore-protocol stream on the websocket
- Open a channel for the key that was fetched before
- Create a browser-side hypercore using as storage a proxy with a javascript [random-access-idb](https://github.com/random-access-storage/random-access-idb) implementation for the key that was fetched before
- Replicate all data blocks from the server to the browser-side WASM hypercore, saving the content to IndexedDB
- Read all replicated blocks from the hypercore and display them on the page (as a string in a `pre` element)

A Node.js server has the simple demo backend:

- Create a hypercore feed in-memory
- Append the contents of this README file
- Open an HTTP server
- On `/key` send the hypercore's key as a hex string
- Serve the static files (index.html, index.js from this dir plus the WASM created through wasm-pack in `/pkg`)
- On other requests, open a websocket connection and pipe it to the replication stream of the hypercore

## How to run

```bash
cargo build --target=wasm32-unknown-unknown
wasm-pack build --dev --target web
npm install
npm run build
npm run start
# open http://localhost:9000
```

If it works, this should display this README in the browser, loaded over hypercore-protocol and hypercore in Rust in WASM :-)

Check the browser console for some logs.
