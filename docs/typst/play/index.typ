#import "/lib/lib.typ": *;
// base_path 定义在 contents.typ,lib.typ 的 `import …: *` 不会把它再导出来。
#import "/lib/contents.typ": *;
// 顶栏左侧那两个选项卡的另一半。toc / with_outline 都关掉:这一页没有
// 目录可列。`with_title: false` 也关掉页面里那行 <h1>:这一页就是一块工具,
// 顶栏的「摆弄」已经标明了身份,再占一行标题只是把编辑器往下挤。
#show: template.with(toc: false, with_outline: false, with_title: false);
// MDUI 的封装都在这儿。
#import "/lib/mdui.typ": mdui-select, mdui-button, mdui-text-field

#let title = [摆弄];

#set raw(lang: "rust");

#custom_element(() => {
    import_style("/play/style.css");
    import_script("/play/script.js");
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
            for x in query(asset).map(x => x.path).filter(x => x.starts-with("/play/demo/")) {
              options.insert(base_path + x, [#x.split("/").last().trim(".rs", at: end)])
            }
            // `<select>` 会自动选中第一个,`<mdui-select>` 不会 —— 它得显式给
            // `value`。不给的话框里是空的,页面一进来就没法直接「载入」。
            mdui-select(options, value: options.keys().at(0), id: "play-sample", variant: "outlined")
          }
          #mdui-button(id: "play-sample-load", variant: "outlined", icon: "download")[载入]
        ]
        // 「取消」和「编译」是一对动作 —— 取消取消的就是正在跑的那次编译,
        // 所以贴在它旁边;「下载」属于右栏的结果,不在这组里。
        #html_elem("span", class: "play-run-group")[
          #mdui-button(id: "play-cancel", variant: "outlined", icon: "close", hidden: true)[取消]
          #mdui-button(id: "play-run", variant: "filled", icon: "play_arrow")[编译]
        ]
      ]
      #mdui-text-field(id: "play-editor", variant: "outlined", textarea: true, rows: "20")
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
