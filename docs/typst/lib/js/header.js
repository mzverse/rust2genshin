// 顶栏是全站 chrome,不该寄生在「本页目录」那个组件里,所以它独立编译成
// lib/header.html,这里在运行时抓进来渲染 —— 和 lib/js/toc.js 抓
// lib/toc.html 是同一套路(同 arch/docs/custom-element 那一页)。
//
// 代价是 shadow 边界:outline.js / theme.js 拿 document.querySelector
// 找不到里面的按钮。所以这里只负责「收到点击」,然后往 document 上发
// 事件,由那两个脚本各自去开抽屉 / 切主题。反向(主题变了)走
// `theme-applied`。这套单向通信跟 toc.js 的 `drawer-close` 是一个路子。
//
// 整段包在 IIFE 里:这些都是普通 script,共享同一个全局作用域,而
// toc.js 也在顶层 `const pathHtml` —— 同名会直接抛 "Identifier 'pathHtml'
// has already been declared",整份脚本连带后面的都不执行。
(() => {

const pathHtml = new URL('../header.html', document.currentScript.src).pathname;
const pathCss = new URL('../css/header.css', document.currentScript.src).pathname;

// 「摆弄」分区怎么算:摆弄页单独编译成 /play.html(见 docs/src/typst.rs
// 的 bundle_document),所以拿 new URL('../../play', 本脚本) 解出它的位置,
// 再把 .html 后缀抹掉 —— 否则 /play 和 /play.html 会当成两处。
const stripHtml = p => p.replace(/\.html$/, '');
const playPath = stripHtml(new URL('../../play', document.currentScript.src).pathname);
const isUnder = (path, root) => path === root || path.startsWith(root + '/');

customElements.define('site-header', class extends HTMLElement {
    constructor() {
        super();
        this.attachShadow({ mode: 'open' });
    }

    connectedCallback() {
        void this.render();
    }

    async render() {
        const [html, css] = await Promise.all([
            fetch(pathHtml).then(r => r.text()),
            fetch(pathCss).then(r => r.text()),
        ]);
        // header.html 是一整页(走的是和别的页面一样的 template),head 里还
        // 挂着全站那些 <link> 和 <custom-element> 的注册模板。直接丢给
        // innerHTML 的话它们会被当成 shadow 的内容解析进来 —— 那些
        // <link> 是真的会往 shadow root 里加样式表的。所以先让 DOMParser
        // 走一遍正规解析,只取 body。
        const sheet = new CSSStyleSheet();
        sheet.replaceSync(css);
        this.shadowRoot.adoptedStyleSheets.push(sheet);
        this.shadowRoot.innerHTML = new DOMParser()
            .parseFromString(html, 'text/html')
            .body.innerHTML;
        this.wire();
    }

    wire() {
        const root = this.shadowRoot;

        // 这一页到底有没有侧栏。有就留着两颗 ☰,没有(比如「摆弄」分区
        // 用的是 `template.with(toc: false, with_outline: false)`)就把它们
        // 藏掉 —— 点了没有面板可开,留着是死按钮。
        // 判据在 light DOM,而这里的样式表穿不过 shadow 边界,所以把结论
        // 记在宿主元素上,让 header.css 用 `:host([...])` 去读。
        if (!document.querySelector('toc-component, .outline')) {
            this.dataset.noDrawers = '';
        }

        // 分区选中态:在摆弄分区里(或它下面)选「摆弄」,其余一律「文档」。
        // 认的是 location.pathname,不是页面上打的标记 —— 后者在 html
        // 导出下要每页手写一遍,漏一页就错一页。
        const active = isUnder(stripHtml(location.pathname), playPath) ? 'play' : 'docs';
        root.querySelectorAll('.site-tab').forEach(tab => {
            const on = tab.dataset.tab === active;
            tab.classList.toggle('active', on);
            if (on) tab.setAttribute('aria-current', 'page');
            else tab.removeAttribute('aria-current');
        });
        // aria-label 说「按下去会变成什么」。theme.js 已经在 DOMContentLoaded
        // 之前把主题定下来了,这里读一次就是当前状态。
        const themeButton = root.querySelector('.theme-toggle');
        const themeLabel = () => (document.documentElement.dataset.theme === 'dark'
            ? '切换到浅色模式' : '切换到深色模式');
        if (themeButton) {
            themeButton.setAttribute('aria-label', themeLabel());
            themeButton.addEventListener('click', () => {
                document.dispatchEvent(new CustomEvent('theme-toggle'));
            });
            // 主题还可能被系统设置改掉(theme.js 跟着系统走),不只是被点。
            document.addEventListener('theme-applied', () => {
                themeButton.setAttribute('aria-label', themeLabel());
            });
        }

        for (const cls of ['toc-toggle', 'outline-toggle']) {
            const button = root.querySelector('.' + cls);
            if (button) {
                button.addEventListener('click', () => {
                    document.dispatchEvent(new CustomEvent('drawer-toggle', {
                        detail: { which: cls === 'toc-toggle' ? 'toc' : 'outline' },
                    }));
                });
            }
        }
    }
});

})();