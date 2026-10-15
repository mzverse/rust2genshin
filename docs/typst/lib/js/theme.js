// 在第一帧之前定下主题。这是 <head> 里的阻塞脚本,跑在正文渲染之前,
// 所以 data-theme 从一开始就是对的 —— 不用 inline script,也不用
// document.write,localStorage 抛错也不会把页面卡在错的主题上。
//
// 深色模式因此依赖 JS。侧栏、目录抽屉、节点图、复制按钮本来就全是 JS
// 驱动的,没 JS 的读者今天也拿不到这个站。
const KEY = 'r2g-theme';
const root = document.documentElement;

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
}

const saved = stored();
apply(saved === 'dark' || saved === 'light' ? saved : system());

// 跟着系统实时变 —— 但只在读者没有手动选过的时候。没有这道闸,手动切到
// 浅色之后再改系统设置,会把读者的选择悄悄冲掉。
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', e => {
    if (!stored()) apply(e.matches ? 'dark' : 'light');
});

// 按钮是 Typst 写进 body 的,脚本在 head 里跑的时候它还不存在。
const LABEL = { dark: '切换到浅色模式', light: '切换到深色模式' };

document.addEventListener('DOMContentLoaded', () => {
    const button = document.querySelector('.theme-toggle');
    if (!button) return;
    // aria-label 说的是「按下去会变成什么」,不是「现在是什么」。
    button.setAttribute('aria-label', LABEL[root.dataset.theme]);
    button.addEventListener('click', () => {
        const next = root.dataset.theme === 'dark' ? 'light' : 'dark';
        try {
            localStorage.setItem(KEY, next);
        } catch {
            /* 不持久,下次访问回到系统设置 */
        }
        apply(next);
        button.setAttribute('aria-label', LABEL[next]);
    });
});
