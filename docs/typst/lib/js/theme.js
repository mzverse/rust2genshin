// 在第一帧之前定下主题。这是 <head> 里的阻塞脚本,跑在正文渲染之前,
// 所以 data-theme 从一开始就是对的 —— 不用 inline script,也不用
// document.write,localStorage 抛错也不会把页面卡在错的主题上。
//
// 深色模式因此依赖 JS。侧栏、目录抽屉、节点图、复制按钮本来就全是 JS
// 驱动的,没 JS 的读者今天也拿不到这个站。
const KEY = 'r2g-theme';
const root = document.documentElement;

// 本次访问里读者点过按钮的标记。只活在内存里,刷新就没了 —— 所以它
// 只能覆盖当前这一次页面浏览,跨访问的偏好还得靠 stored()。
let manual = false;

function system() {
    return matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

// 某些 file:// 和隐私配置下 localStorage 直接抛。那就让主题不持久,
// 而不是让整个脚本挂掉。
function stored() {
    try {
        return localStorage.getItem(KEY);
    } catch {
        return null;
    }
}

function apply(theme) {
    root.dataset.theme = theme;
    // Shoelace(摆弄页那套组件)不认 data-theme:它的深色 token 挂在
    // `.sl-theme-dark` 这个 class 下面(dark.css),class 不加就一直是浅色。
    // 放在 <html> 上,整页的 sl-* 组件一起吃到。
    root.classList.toggle('sl-theme-dark', theme === 'dark');
    // 顶栏那顆按钮在 shadow root 里,只能靠这个事件同步它的 aria-label。
    // 初次 apply 在 header.js 之前发,那时没人听 —— 无所谓,header.js 建好
    // shadow 时自己读一次 dataset.theme。
    document.dispatchEvent(new CustomEvent('theme-applied', { detail: { theme } }));
}

const saved = stored();
apply(saved === 'dark' || saved === 'light' ? saved : system());

// 跟着系统实时变 —— 但只在读者没有手动选过的时候。手动选过有两道信号:
// manual 管这一次页面浏览(点按钮就置位,即使存储被禁用也能挡住),
// stored() 管之后的访问(刷新后 manual 归零,但存下来的偏好还在)。
// 两个都没有才跟着系统走;少了任何一道,手动切过的读者都可能在自己的
// 选择被系统设置悄悄冲掉 —— 存储不可用时缺 manual,刷新后缺 stored()。
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', e => {
    if (!manual && !stored()) apply(e.matches ? 'dark' : 'light');
});

// 按钮在 <site-header> 的 shadow root 里(header.js 渲染的那一页),这里
// 的 document.querySelector 看不见它。所以按钮那边不直接改主题,而是往
// document 上发一个 `theme-toggle`,由这里接手;每次真的换了主题再发回
// 一个 `theme-applied`,让按钮去改自己的 aria-label。
// aria-label 说的是「按下去会变成什么」,不是「现在是什么」。
document.addEventListener('theme-toggle', () => {
    // 先记下这次手动选择,哪怕下面写存储失败了。
    manual = true;
    const next = root.dataset.theme === 'dark' ? 'light' : 'dark';
    try {
        localStorage.setItem(KEY, next);
    } catch {
        /* 不持久,下次访问回到系统设置 */
    }
    apply(next);
});
