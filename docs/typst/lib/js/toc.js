const pathHtml = new URL('../toc.html', document.currentScript.src).pathname;
const pathCss = new URL('../css/toc.css', document.currentScript.src).pathname;

customElements.define('toc-component', class extends HTMLElement {
    constructor() {
        super();
        this.attachShadow({ mode: 'open' });
    }

    connectedCallback() {
        void(this.loadToc());
    }

    async loadToc() {
        async function get(path) {
            return await (await fetch(path)).text();
        }
        let html = get(pathHtml);
        let css = get(pathCss);

        const extraSheet = new CSSStyleSheet();
        extraSheet.replaceSync(await css);
        this.shadowRoot.adoptedStyleSheets.push(extraSheet);
        this.shadowRoot.innerHTML = await html;

        let currentPath = decodeURIComponent(window.location.pathname);
        if(currentPath.endsWith("/"))
            currentPath = currentPath.substring(0, currentPath.length - 1);
        this.shadowRoot.querySelectorAll('a').forEach(link => {
            const href = link.getAttribute('href');
            if (href === currentPath || href === currentPath + "/index.html") {
                link.closest('li').classList.add('current');
                let parent = link.closest('*:has(ul)');
                while (parent) {
                    parent.classList.add('open');
                    parent = parent.parentElement?.closest('*:has(ul)');
                }
            }
        });
        this.shadowRoot.querySelectorAll('li > button.open').forEach(item => {
            item.addEventListener('click', e => {
                e.preventDefault();
                e.target.parentElement.classList.toggle('open');
            });
        });
        // 抽屉模式的关闭按钮。开关逻辑在 outline.js(它管着遮罩和 body 的
        // overflow),这里只发一个事件过去,不重复实现一遍。
        this.shadowRoot.querySelectorAll('.drawer-close').forEach(item => {
            item.addEventListener('click', () => {
                document.dispatchEvent(new CustomEvent('drawer-close'));
            });
        });
    }
});
