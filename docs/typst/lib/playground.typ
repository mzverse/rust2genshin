#import "/lib/lib.typ": *;

// 摆弄页的主体。app 的状态全在 JS 里,Typst 只出一个宿主标签加一份
// shadow root 的模板原型 —— 和 card / expand-panel 同一个路子。
// `backend` 落到宿主标签的 `data-backend` 上,JS 读它当初始值。默认值
// 留在 JS 里(本机 `cargo run` 的场景),但**站点应该显式传自己的地址** ——
// 线上摆弄页的用户不会知道要手填 `wss://…`,不填就只有一句「连不上后端」。
#let playground(
  backend: none,
) = custom_element(
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
        #html_elem("span", part: "hint")[改这个要重新加载页面;地址存在 localStorage 里]
      ]
    ]
    import_script("/lib/js/playground.js");
    import_style("/lib/css/playground.css");
  },
  if backend == none {
    html_elem("playground-app")
  } else {
    html_elem("playground-app", data-backend: backend)
  },
)
