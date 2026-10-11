document.addEventListener('DOMContentLoaded', _ => {
    function initializeScrollHighlight(container) {
        const associations = Array.from(container.querySelectorAll("li")).reduce((acc, listItem) => {
            const anchorElement = listItem.querySelector("a");
            const targetId = anchorElement.getAttribute("href").substring(1);
            const targetElement = document.getElementById(targetId);
            if (targetElement) {
                acc.push({
                    item: listItem,
                    target: targetElement,
                    anchor: anchorElement
                });
            }
            return acc;
        }, []);
        const updateCurrentHighlight = () => {
            const currentVisible = associations.find(assoc => isElementInViewport(assoc.target));
            if (currentVisible) {
                associations.forEach(assoc => assoc.item.classList.remove("current"));
                currentVisible.item.classList.add("current");
            }
        };
        updateCurrentHighlight();
        window.addEventListener("scroll", updateCurrentHighlight);
        window.addEventListener("resize", updateCurrentHighlight);
    }

    function isElementInViewport(element) {
        const rect = element.getBoundingClientRect();
        return [
            rect.top >= 0,
            rect.left >= 0,
            !(rect.bottom > (window.innerHeight || document.documentElement.clientHeight)),
            !(rect.right > (window.innerWidth || document.documentElement.clientWidth))
        ].every(b => b);
    }

    const overviewList = document.querySelector(".outline > ul");
    if (overviewList) {
        initializeScrollHighlight(overviewList);
    }

    /* 两个抽屉:左栏全站目录、右栏本页目录。窄屏下各自固定定位、默认在
     * 屏幕外,`.open` 滑进来,共用一层遮罩,同一时刻只开一个。
     * `columnAt` 是这个抽屉恢复成 grid 常驻列的断点(跟 outline.css 的
     * media query 对齐)—— 跨过它就把还开着的抽屉关掉,否则遮罩会盖在
     * 常驻列上。 */
    const drawers = [
        { panel: document.querySelector('toc-component'), toggle: '.toc-toggle', columnAt: 1080 },
        { panel: document.querySelector('.outline'), toggle: '.outline-toggle', columnAt: 768 },
    ].filter(d => d.panel);

    let overlay = document.querySelector('.drawer-overlay');
    if (!overlay) {
        overlay = document.createElement('div');
        overlay.className = 'drawer-overlay';
        document.body.appendChild(overlay);
    }

    let opened = null;

    function closeDrawer() {
        if (!opened) return;
        opened.panel.classList.remove('open');
        opened = null;
        overlay.classList.remove('open');
        document.body.style.overflow = '';
    }

    function openDrawer(drawer) {
        if (opened === drawer) return;
        closeDrawer();
        drawer.panel.classList.add('open');
        opened = drawer;
        overlay.classList.add('open');
        document.body.style.overflow = 'hidden';
    }

    for (const drawer of drawers) {
        const toggle = document.querySelector(drawer.toggle);
        if (toggle) toggle.addEventListener('click', () => openDrawer(drawer));
    }
    // 右栏的 ✕ 在页面里,直接接;左栏的 ✕ 在 toc-component 的 shadow root
    // 里查不到,由 toc.js 发 `drawer-close` 事件过来。
    document.querySelectorAll('.outline-close').forEach(btn => {
        btn.addEventListener('click', closeDrawer);
    });
    document.addEventListener('drawer-close', closeDrawer);
    overlay.addEventListener('click', closeDrawer);

    document.addEventListener('keydown', e => {
        if (e.key === 'Escape') closeDrawer();
    });

    let resizeTimer;
    window.addEventListener('resize', () => {
        clearTimeout(resizeTimer);
        resizeTimer = setTimeout(() => {
            if (opened && window.innerWidth >= opened.columnAt) closeDrawer();
        }, 100);
    });
});
