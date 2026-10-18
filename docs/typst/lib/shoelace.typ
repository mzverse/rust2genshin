#import "/lib/lib.typ": *

/// 把 Shoelace 引进来。**每个用到 `<sl-*>` 的地方都要调一次** ——
/// `custom_element` 的作用就是把回调挂到那一页的 `<head>` 上,没调就等于没引。
///
/// 只注册 `sl-button` 一个模块,不用 `shoelace-autoloader.js`:autoloader 会挂
/// 一个 MutationObserver 盯着整页、按标签名懒加载,这一页只有按钮,多那一层
/// 没必要;而且它是 ES module,`import_script` 发的是普通 `<script>`,也发不出
/// `type="module"`。
#let import_shoelace() = {
    // light.css 挂在 `:root`,dark.css 挂在 `.sl-theme-dark`。先 light 后 dark:
    // `<html class="sl-theme-dark">` 时两条规则特异性相同(都是 0,1,0),
    // 后加载的 dark 赢。切换在 lib/js/theme.js 里。
    import_style("https://cdn.jsdelivr.net/npm/@shoelace-style/shoelace@2.20.1/cdn/themes/light.css");
    import_style("https://cdn.jsdelivr.net/npm/@shoelace-style/shoelace@2.20.1/cdn/themes/dark.css");
    // 只覆盖主色 / 尺寸 / 圆角 / 字体,不重画 Shoelace 的中性色阶。
    import_style("/lib/css/shoelace-theme.css");
    // `<i class="material-icons">` 只是把图标名交给 Material Icons 这个**字体**
    // 去渲染,字体没引的话位置上显示的是图标名那几个字母。
    import_style("https://fonts.googleapis.com/icon?family=Material+Icons");
    html_elem("script", attrs: (
        "type": "module",
        "src": "https://cdn.jsdelivr.net/npm/@shoelace-style/shoelace@2.20.1/cdn/components/button/button.js",
    ))[]
}

/// `<sl-button>`。
///
/// `variant`:`default`(描边)/ `primary`(实心主色)/ `text`(无边框)。
///
/// 无值属性(布尔属性)在 HTML 里写成 `attr=""`,不是省略值 —— 站点的
/// `html_elem` 包装要求属性值是字符串,给 `none` 会报
/// `expected string, found none`。
#let sl-button(
    body,
    id: none,
    variant: "default",
    icon: none,
    disabled: false,
    hidden: false,
) = {
    custom_element(
        import_shoelace,
        html_elem(
            "sl-button",
            // `prefix` slot 是组件留的图标位。图标字体引在 `import_shoelace` 里。
            ..if icon != none {
                (html_elem("i", class: "material-icons", attrs: ("slot": "prefix"))[#icon],)
            } else {},
            body,
            id: id,
            variant: variant,
            ..if disabled { ("disabled": "") } else {},
            ..if hidden { ("hidden": "") } else {},
        ),
    )
}
