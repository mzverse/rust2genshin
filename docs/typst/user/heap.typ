#import "/lib/lib.typ": *;
#show: template;

#let title = [堆分配];

#set raw(lang: "rust");

#import "/lib/card.typ": *;

主要用于*储存引用*或自引用，包括借用

`Box`依赖于节点图变量，不可跨节点图使用

#card_attention[
    挂在*不同实体*上的节点图也互不相通，即使来自同一个元件也不同

    因此涉及同一个`Box`的事物应由一个实体统一处理
]

= 构造

使用`Box::new`或直接`into()`，分配一个空间并将已有值*移动*进去

= 析构

当你丢弃一个`Box`时，它会自动析构其中的对象，空间也会被自动释放

= 借用

不可变借用`&Box<T>`和可变借用`PartialMut<Box<T>>`均是可储存的

它们本质上和`Box<T>`一样，就是堆上的一个地址
