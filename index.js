import init, { run_async } from './pkg/hypercore_rs_wasm.js'

const ADDRESS = 'ws://localhost:9000'
window.addEventListener('load', async () => {
    try {
        const info = await (await window.fetch('/key')).json()
        const publicKey = Buffer.from(info.key, 'hex');
        console.log('fetched key: ' + info.key)
        await init('./pkg/hypercore_rs_wasm_bg.wasm')
        console.log('hypercore-protocol-wasm loaded')

        console.log('init hypercore-protocol (address %s, key %s)', ADDRESS, info.key)
        await run_async(ADDRESS, info.key)

        // console.log('opening hypercore feed in browser');
        // const feed = hypercore(function (filename) {
        //     const raidbCreator = raidb(filename);
        //     return raidbCreator(`hypercore.db`);
        // }, publicKey);
        // console.log('reading from feed in browser', feed);
        // feed.get(0, { ifAvailable: true }, (err, content) => {
        //     console.log("HELLO", err, content);
        // })
        console.log('finished')
    } catch (err) {
        console.error('ERROR', err)
    }
})
