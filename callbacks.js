const raidb = require('random-access-idb');
const storages = {};
const toBuffer = require('typedarray-to-buffer')

function initStorage(id) {
    if (!(id in storages)) {
        const raidbCreator = raidb(id);
        storages[id] = raidbCreator(`hypercore.db`);
    }
}

export async function storage_write(id, offset, data) {
    initStorage(id);
    return new Promise(resolve => {
        try {
            storages[id].write(Number(offset), toBuffer(data), (err) => {
                if (err) throw err;
                resolve();
            });
        } catch(exp) {
            console.log(exp);
            throw exp;
        }

    });
}

export async function storage_read(id, offset, length) {
    initStorage(id);
    return new Promise(resolve => {
        storages[id].read(Number(offset), Number(length), (err, data) => {
            if (err) throw err;
            const returnData = data ? data : new Uint8Array();
            resolve(returnData);
        });
    });
}

export async function storage_del(id, offset, length) {
    return new Promise(resolve => resolve());
}

export async function storage_truncate(id, length) {
    return new Promise(resolve => resolve());
}

export async function storage_len(id) {
    return new Promise(resolve => resolve());
};

export async function storage_is_empty(id) {
    return new Promise(resolve => resolve());
};

export async function storage_sync_all(id) {
    return new Promise(resolve => resolve());
}
