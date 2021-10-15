const raw = require('random-access-web');

export async function testing_name() {
    console.log("CALLING NAME");
    console.log(raw);
    return new Promise(resolve => {
        setTimeout(() => resolve('rust'), 3000);
    });
}

export async function storage_write(id, offset, data) {
    return new Promise(resolve => resolve());
}

export async function storage_read(id, offset, length) {
    return new Promise(resolve => resolve());
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
