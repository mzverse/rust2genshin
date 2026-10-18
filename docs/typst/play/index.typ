#import "/lib/lib.typ": *;
// base_path 定义在 contents.typ,lib.typ 的 `import …: *` 不会把它再导出来。
#import "/lib/contents.typ": *;
// 顶栏左侧那两个选项卡的另一半。toc / with_outline 都关掉:这一页没有
// 目录可列。`with_title: false` 也关掉页面里那行 <h1>:这一页就是一块工具,
// 顶栏的「摆弄」已经标明了身份,再占一行标题只是把编辑器往下挤。
#show: template.with(toc: false, with_outline: false, with_title: false);
// MDUI 的封装都在这儿。
#import "/lib/mdui.typ": mdui-select, mdui-button

#let title = [摆弄];

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
    // ---- 左栏:工具条 + 编辑框 ----
    #html_elem("div", class: "play-left")[
      #html_elem("div", class: "play-left-head")[
        // 「示例 + 选 + 载入」框成一组:它是「往编辑器里放段现成代码」,
        // 和右边那组「取消 / 编译」是两件事。
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
            // 用原生 `<select>` 而不是 `<mdui-select>`(和下面的「目标」同一个
            // 理由):MDUI 组件的高度在它的 shadow root 里写死(`mdui-text-field`
            // 内层 `padding: 0.875rem`、`mdui-button` 的 `:host { height: 2.5rem }`),
            // 整条链上没有尺寸变量可用,唯一能穿透的 typescale 只管字号。样例下拉
            // 因此天生 48px、按钮 40px,同一行里对不齐 —— 换原生控件后两者都由
            // `style.css` 的 `--play-ctl-h` 一处控制。
            // `<select>` 自动选中第一项,不用显式给 value。
            html_elem("select", id: "play-sample", class: "play-target play-sample-select")[
              #for (path, label) in options.pairs() {
                html_elem("option", value: path)[#label]
              }
            ]
          }
          #mdui-button(id: "play-sample-load", variant: "outlined", icon: "download")[载入]
        ]
        // 「目标」选择器由 `Hello.targets` 驱动 —— 不写死选项,后端报什么就
        // 渲染什么;显示名(`超限模式` / `经典模式`)在 script.js 里查
        // TARGET_LABELS。默认选第一项(`Hello.targets` 的顺序就是显示顺序)。
        // 用原生 `<select>` 而不是 `<mdui-select>`:这个选择器只有两三行,
        // mdui 的样式包袱反而让头部多出一截高度,不值。
        #html_elem("div", class: "play-picker play-picker-target")[
          #html_elem("span", class: "play-label")[目标：]
          #html_elem("select", id: "play-target", class: "play-target")[]
        ]
        // 「取消」和「编译」是一对动作 —— 取消取消的就是正在跑的那次编译,
        // 所以贴在它旁边;「下载」属于右栏的结果,不在这组里。
        #html_elem("span", class: "play-run-group")[
          #mdui-button(id: "play-cancel", variant: "outlined", icon: "close", hidden: true)[取消]
          #mdui-button(id: "play-run", variant: "filled", icon: "play_arrow")[编译]
        ]
      ]
      // CodeMirror 6 挂载点。`<mdui-text-field textarea>` 的内层 textarea 在 shadow
      // root 里,JS 取不到 `selectionStart`、dispatch 也得绕一圈;直接挂到普通 div
      // 上省事,而且 CodeMirror 自己管光标、IME、composition event。
      #html_elem("div", id: "play-editor", class: "play-editor")[]
    ]

    // 拖动它改左右宽度。role/aria 只是让读屏知道这是个分隔条。
    #html_elem("div", class: "play-splitter", id: "play-splitter", attrs: (
      "title": "拖动调整左右宽度",
      "role": "separator",
      "aria-orientation": "vertical",
    ))[]

    // ---- 右栏:选项卡条 + 各页内容 ----
    #html_elem("div", class: "play-right")[
      #html_elem("div", class: "play-tabs-bar")[
        // 原生 <button> 而不是 MDUI 的:选项卡要的是「一排贴着底边的文字」,
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
          #mdui-button(id: "play-download", variant: "outlined", icon: "save_alt", hidden: true)[下载 .gia]
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
]
