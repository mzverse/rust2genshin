{
    const template = document.getElementById("code-block");
    const icons = {
        "html": "html5",
        "css": "css3-alt",
        "javascript": "js",
        "c++": "cpp",
        "C#": "csharp",
        "go": "golang",
        "ts": "typescript",
        "vue": "vuejs"
    };
    customElements.define("code-block", class extends HTMLElement {
        constructor() {
            super();
            this.attachShadow({ mode: 'open' });
        }
        connectedCallback() {
            this.shadowRoot.appendChild(template.content.cloneNode(true));
            let badge = this.shadowRoot.querySelector('[part="language-badge"]');
            this.shadowRoot.querySelector('slot').addEventListener('slotchange', () => {
                let lang = this.firstChild.firstChild.getAttribute("data-lang");
                badge.firstChild?.remove();
                badge.append(
                    Object.assign(document.createElement('i'), { className: `fab mr-1 fa-${icons[lang] ?? lang}` }),
                    lang
                );
            });
            const button = this.shadowRoot.querySelector('[part="copy"]');
            button.addEventListener('click', () => {
                navigator.clipboard.writeText(this.textContent).then(() => {
                    if(button.classList.contains('copied'))
                        return;
                    const original = Array.from(button.childNodes);
                    button.replaceChildren(Object.assign(document.createElement('i'), { className: 'fas fa-check' }), ' 已复制');
                    button.classList.add('copied');
                    setTimeout(() => {
                        button.replaceChildren(...original);
                        button.classList.remove('copied');
                    }, 2000);
                }).catch(err => {
                    console.error('复制失败：', err);
                });
            });
        }
    });
}

// document.addEventListener('DOMContentLoaded', _ => {
//     hljs.configure({
//         cssSelector: 'code, code-block',
//         ignoreUnescapedHTML: true,
//     });
//     hljs.registerLanguage("output", () => ({}))
//     hljs.highlightAll();
// });
