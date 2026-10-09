#import "/lib/lib.typ": *;
#let title = [卡片];
#show: template;

= 导入依赖

```typ
#import "/lib/card.typ": *;
```

= 插入卡片

```typ
#card_info[
    信息卡片
]
#card_tip[
    提示卡片
]
#card_attention[
    警告卡片
]
```

#import "/lib/card.typ": *;

#card_info[
    信息卡片
]
#card_tip[
    提示卡片
]
#card_attention[
    警告卡片
]
