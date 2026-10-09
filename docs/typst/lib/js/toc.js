const tocPath = new URL('../toc.html', document.currentScript.src).pathname;

customElements.define('toc-component', class extends HTMLElement {
    constructor() {
        super();
        this.attachShadow({ mode: 'open' });
    }

    connectedCallback() {
        void(this.loadToc());
    }

    async loadToc() {
        const response = await fetch(tocPath);
        this.shadowRoot.innerHTML = await response.text();

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
    }
});
