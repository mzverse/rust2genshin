#import "/lib/lib.typ": *;
// base_path 定义在 contents.typ,lib.typ 的 `import …: *` 不会把它再导出来。
#import "/lib/contents.typ": *;
// 顶栏左侧那两个选项卡的另一半。toc / with_outline 都关掉:这一页没有
// 目录可列。`with_title: false` 也关掉页面里那行 <h1>:这一页就是一块工具,
// 顶栏的「摆弄」已经标明了身份,再占一行标题只是把编辑器往下挤。
#show: template.with(toc: false, with_outline: false, with_title: false);
// Shoelace 的封装都在这儿。
#import "/lib/shoelace.typ": sl-button, sl-divider, sl-select, sl-option

#let title = [摆弄];
#let tag = <tag-playground>;

#set raw(lang: "rust");

// script 标签要 `type="module"`:`script.js` 里有 ESM 的 `import … from "https://esm.sh/..."`,
// 普通脚本会被解析成「未声明的 import 语句」直接报错。`import_script` 助手发的是
// `<script src="...">`,不带 `type`,所以**不能**用它 —— 直接走 `html_elem`,
// 顺手加上 `./lsp-client.js` 也在同一目录下,模块 base 解析会把它正确指向。
#custom_element(() => {
    import_style("/play/style.css");
    html_elem("script", attrs: (
        "type": "module",
        "src": base_path + "/play/script.js",
    ))[]
})[]

// 页面里**不出现**后端地址,也不给改:地址表写死在 script.js 的
// `BACKENDS` 里。
#html_elem("div", class: "play")[
  // 左右两栏 + 中间那条可拖动的分隔条(`grid-template-columns` 是
  // `左 | 8px | 右`)。宽度由 script.js 拖拽时写到 `--play-left` 上。
  #html_elem("div", class: "play-layout", id: "play-layout")[
    // ---- 五格平铺:工具条 | 编辑框 | 拖动条 | 选项卡条 | 内容 ----
    //
    // 左栏的两个块之间、右栏的两个块之间**不套 wrapper**:它们本来就是
    // 同一个 grid 里同一行(表头)的两格,`auto` 行会自然取两边更高的那个,
    // 对齐是白送的。平铺之后表头↔内容的间隙交给父级一条 `row-gap`,
    // 左右两栏同时吃到,不像套了 wrapper 之后各写各的还要防一边漏掉。
    // DOM 顺序也正好是窄屏上下堆叠要的顺序。
    #html_elem("div", class: "play-left-head")[
        // 「示例 + 选 + 载入」用一条短竖线和「目标」隔开,不再各套一个边框
        // 盒子 —— 工具条上两个带框的组并排,看起来像两块独立面板。
        #html_elem("div", class: "play-picker")[
          #html_elem("span", class: "play-label")[示例：]
          // 样例是**静态资产**(`play/demo/*.rs` 会被当 asset 拷进输出目录),
          // 不从后端取 —— 页面挂了样例也还在,不用等后端起来。
          // `value` 放真路径、显示名(去掉目录和 `.rs`)做标签正文,script.js
          // 直接拿 `.value` 去 fetch。
          #context {
            // 键是真路径、值是显示名。`("a/b": …)` 这种带斜杠的键必须加引号。
            let options = (:)
            // bundler(`docs/src/typst.rs` 的 `walk`)已经把资源排好序了,
            // 这里再排一次是为了不把下拉框的顺序押在它身上。排完才是字典序 ——
            // 文件名的 `aaa_` / `aab_` 前缀就是靠这个排序生效的。
            for x in query(asset).map(x => x.path).filter(x => x.starts-with("/play/demo/")).sorted() {
              options.insert(base_path + x, [#x.split("/").last().trim(".rs", at: end)])
            }
            // 用 `<sl-select>` 而不是原生 `<select>`:和按钮同一套主题、
            // 同一个 `--sl-input-height-medium`(见 `style.css` 的
            // `.play-layout`),胶囊形也由组件的 `pill` 属性保证。
            // 原生 select 没有「选中第一项」之外的空值态,这里显式给 value。
            let pairs = options.pairs()
            sl-select(
              id: "play-sample",
              class: "play-select play-sample-select",
              value: if pairs.len() > 0 { pairs.first().at(0) } else { none },
              aria_label: "示例",
              [
                #for (path, label) in pairs {
                  sl-option(path, [#label])
                }
              ],
            )
          }
          #sl-button(id: "play-sample-load", variant: "default", icon: "download")[载入]
        ]
        #sl-divider(vertical: true, class: "play-divider")
        // 「目标」选择器由 `Hello.targets` 驱动 —— 不写死选项,后端报什么就
        // 渲染什么;显示名(`超限模式` / `经典模式`)在 script.js 里查
        // TARGET_LABELS。默认选第一项(`Hello.targets` 的顺序就是显示顺序)。
        // 选项由后端 `hello` 填(见 script.js),初始空值显示 placeholder。
        #html_elem("div", class: "play-picker")[
          #html_elem("span", class: "play-label")[目标：]
          #sl-select(
            id: "play-target",
            class: "play-select play-select-target",
            placeholder: "未连接",
            aria_label: "目标",
          )[]
        ]
        // 「取消」和「编译」是一对动作 —— 取消取消的就是正在跑的那次编译,
        // 所以贴在它旁边;「下载」属于右栏的结果,不在这组里。
        #html_elem("span", class: "play-run-group")[
          #sl-button(id: "play-cancel", variant: "default", icon: "close", hidden: true)[取消]
          #sl-button(id: "play-run", variant: "primary", icon: "play_arrow")[编译]
        ]
      ]
      // CodeMirror 6 挂载点。挂普通 div:把编辑器塞进组件(比如 textarea)的
      // shadow root 里,JS 取不到 `selectionStart`、dispatch 也得绕一圈;直接
      // 挂外面省事,而且 CodeMirror 自己管光标、IME、composition event。
      #html_elem("div", id: "play-editor", class: "play-editor")[]

    // 拖动它改左右宽度。role/aria 只是让读屏知道这是个分隔条。
    #html_elem("div", class: "play-splitter", id: "play-splitter", attrs: (
      "title": "拖动调整左右宽度",
      "role": "separator",
      "aria-orientation": "vertical",
    ))[]

    // ---- 右栏:选项卡条 + 各页内容 ----
    #html_elem("div", class: "play-tabs-bar")[
        // 原生 <button> 而不是 `<sl-button>`:选项卡要的是「一排贴着底边的文字」,
        // 用按钮组件反而得再把它压扁回文字的样子。
        #html_elem("div", class: "play-tabs")[
          #html_elem("button", type: "button", class: "play-tab", attrs: (
            "data-tab": "log",
            "data-active": "",
          ))[日志]
          #html_elem("button", type: "button", class: "play-tab", attrs: (
            "data-tab": "preview",
          ))[预览]
        ]
        // 状态行跟在选项卡后面、下载按钮前面。它会挤占中间那段空白,
        // 太长的文案由 CSS 省略。
        #html_elem("span", id: "play-status", class: "play-status")[]
        #html_elem("span", class: "play-actions")[
          #sl-button(id: "play-download", variant: "default", icon: "save_alt", hidden: true)[下载 .gia]
        ]
      ]
      #html_elem("div", class: "play-panels")[
        #html_elem("div", class: "play-panel", id: "play-panel-log")[
          #html_elem("pre", id: "play-log", class: "play-log")
        ]
        #html_elem("div", class: "play-panel", id: "play-panel-preview", attrs: ("hidden": ""))[
          #html_elem("div", class: "play-preview")[
            节点图预览还在做,敬请期待。
          ]
        ]
      ]
  ]
]
