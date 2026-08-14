// Service Worker：静态资源缓存，离线可打开已访问页面
// 【教学：SW 生命周期】
//   install：安装时打开缓存空间（caches.open）
//   fetch：每次请求都经过这里——先查缓存，命中直接返回；
//          未命中走网络，成功后把响应存进缓存
// 数据请求（/admin/backup 等）不缓存——只缓存 GET 静态资源。
const CACHE = 'train-record-v1';
self.addEventListener('install', (e) => {
    e.waitUntil(caches.open(CACHE));
});
self.addEventListener('fetch', (e) => {
    if (e.request.method !== 'GET') return;
    e.respondWith(
        caches.match(e.request).then((hit) =>
            hit || fetch(e.request).then((resp) => {
                const copy = resp.clone();
                caches.open(CACHE).then((c) => c.put(e.request, copy));
                return resp;
            })
        )
    );
});
