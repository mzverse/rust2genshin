#import "/lib/lib.typ": *;

// 摆弄页的主体。app 的状态全在 JS 里,Typst 只出一个宿主标签加一份
// shadow root 的模板原型 —— 和 card / expand-panel 同一个路子。
#let playground() = custom_element(
  () => {
    html_elem("template", id: "playground")[
      #html_elem("div", part: "toolbar")[
        #html_elem("button", part: "sample", type: "button")[载入样例]
        #html_elem("button", part: "run", type: "button")[编译]
        #html_elem("button", part: "cancel", type: "button", hidden: "")[取消]
        #html_elem("button", part: "download", type: "button", hidden: "")[下载 .gia]
      ]
      #html_elem("div", part: "layout")[
        #html_elem("textarea", part: "editor", spellcheck: "false", autocomplete: "off")
        #html_elem("div", part: "right")[
          #html_elem("div", part: "status")[]
          #html_elem("pre", part: "log")
        ]
      ]
      #html_elem("div", part: "backend")[
        #html_elem("label")[
          后端地址
          #html_elem("input", part: "backend-input", type: "text")
        ]
        #html_elem("span", part: "hint")[默认连本机 8787;线上要填 wss:\/\/ 的地址]
      ]
    ]
    import_script("/lib/js/playground.js");
    import_style("/lib/css/playground.css");
  },
  html_elem("playground-app"),
)
