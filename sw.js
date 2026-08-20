// Service Worker：静态资源缓存，离线可打开已访问页面
// 【教学：SW 生命周期】
//   install：安装时把关键页面预缓存进 caches（caches.open + addAll）
//   activate：清理旧版本缓存（版本号 CACHE 变化时）
//   fetch：每次请求都经过这里——按请求类型分流（M7 第 5 步）：
//     /static/ 开头 → Cache-first（静态资源不变，命中直接返回）
//     其余 GET    → Network-first（先网络，成功存缓存；失败回缓存）
//
// ⚠️ M6 实际验证发现的两个坑：
//   1. 注册必须显式 { scope: '/' }，否则 SW 只管 /static/ 管不到页面导航
//   2. 光有 fetch 缓存不够——首次注册的页面还没被 SW 控制，
//      导航请求从未经过 SW 就进不了缓存；必须 install 时 addAll 预缓存
// 数据请求（/admin/backup 等）不缓存——只缓存 GET 静态资源 + 关键页面。
const CACHE = 'train-record-v2';
const PRECACHE = [
    '/',
    '/today',
    '/static/style.css',
    '/static/manifest.json',
    '/static/icon-192.png',
    '/static/icon-512.png',
];

self.addEventListener('install', (e) => {
    e.waitUntil(
        caches.open(CACHE).then((c) => c.addAll(PRECACHE))
    );
    self.skipWaiting();
});

self.addEventListener('activate', (e) => {
    e.waitUntil(
        caches.keys().then((keys) =>
            Promise.all(
                keys.filter((k) => k !== CACHE).map((k) => caches.delete(k))
            )
        )
    );
    self.clients.claim();
});

// Cache-first：静态资源（版本化 URL，变了就换 URL，缓存无需失效）
function cacheFirst(e) {
    return caches.match(e.request).then((hit) =>
        hit ||
        fetch(e.request)
            .then((resp) => {
                if (resp.ok || resp.type === 'opaque') {
                    const copy = resp.clone();
                    caches.open(CACHE).then((c) => c.put(e.request, copy));
                }
                return resp;
            })
            .catch(() => Response.error())
    );
}

// Network-first：页面（内容会变，优先网络，失败回缓存）
function networkFirst(e) {
    return fetch(e.request)
        .then((resp) => {
            if (resp.ok || resp.type === 'opaque') {
                const copy = resp.clone();
                caches.open(CACHE).then((c) => c.put(e.request, copy));
            }
            return resp;
        })
        .catch(() =>
            caches.match(e.request).then((hit) =>
                hit ||
                // 离线且未命中：导航请求回退到预缓存的 today 页
                (e.request.mode === 'navigate' ? caches.match('/today') : Response.error())
            )
        );
}

self.addEventListener('fetch', (e) => {
    if (e.request.method !== 'GET') return;
    // 只缓存同源请求，避免缓存第三方（字体/CDN）
    const url = new URL(e.request.url);
    if (url.origin !== self.location.origin) return;

    if (url.pathname.startsWith('/static/')) {
        e.respondWith(cacheFirst(e));
    } else {
        e.respondWith(networkFirst(e));
    }
});
