// Service Worker：静态资源缓存，离线可打开已访问页面
// 【教学：SW 生命周期】
//   install：安装时把关键页面预缓存进 caches（caches.open + addAll）
//   activate：清理旧版本缓存（版本号 CACHE 变化时）
//   fetch：每次请求都经过这里——先查缓存，命中直接返回；
//          未命中走网络，成功后把响应存进缓存
//          网络失败（离线）时：导航请求回退到预缓存的离线页
//
// ⚠️ M6 实际验证发现的两个坑：
//   1. 注册必须显式 { scope: '/' }，否则 SW 只管 /static/ 管不到页面导航
//   2. 光有 fetch 缓存不够——首次注册的页面还没被 SW 控制，
//      导航请求从未经过 SW 就进不了缓存；必须 install 时 addAll 预缓存
// 数据请求（/admin/backup 等）不缓存——只缓存 GET 静态资源 + 关键页面。
const CACHE = 'train-record-v1';
const PRECACHE = [
    '/',
    '/today',
    '/static/manifest.json',
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

self.addEventListener('fetch', (e) => {
    if (e.request.method !== 'GET') return;
    // 只缓存同源请求，避免缓存第三方（字体/CDN）
    const url = new URL(e.request.url);
    if (url.origin !== self.location.origin) return;

    e.respondWith(
        caches.match(e.request).then((hit) =>
            hit ||
            fetch(e.request)
                .then((resp) => {
                    // 只缓存成功响应（200）和可缓存类型
                    if (resp.ok || resp.type === 'opaque') {
                        const copy = resp.clone();
                        caches.open(CACHE).then((c) => c.put(e.request, copy));
                    }
                    return resp;
                })
                .catch(() => {
                    // 离线且未命中：导航请求回退到离线页
                    if (e.request.mode === 'navigate') {
                        return caches.match('/today');
                    }
                    return Response.error();
                })
        )
    );
});
