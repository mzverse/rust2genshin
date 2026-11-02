#import "/lib/lib.typ": *;
#show: template;

#let title = [借用];

#set raw(lang: "rust");

#import "/lib/card.typ": *;

= 共享借用

当`T: Freeze`，共享借用保证被借用的值不会被改变，因此`&T`编译为与`T`相同

当`T: !Freeze`，内部可变，因此`&T`编译为与`PartialMut<T>`相同

#card_info[
    `Cell`这类内部可变性目前不支持，所以 `T: !Freeze` 的情况基本只出现在 `PartialMut` 自身
]

= 可变借用

可以当做共享借用、可以整个赋值、也可以修改其成员

相当于一个变量的setter加上一个`PartialMut<T>`

= PartialMut <user-partial-mut>

可以当做共享借用、也可以修改其成员，但*不能整体赋值*

为方便使用，可`deref_mut`或`into_mut`，但转换得到的`&mut T`仍不能整体赋值

#card_attention[
    *透明布局结构体*编译后等同于其成员，对其成员赋值也等同于整体赋值
]

你不能储存它，如果你需要这么做，考虑将数据包为`Box<T>`，然后储存`PartialMut<Box<T>>`

= Cell

不支持。设计上还有两种走法没定：

- 让 `UnsafeCell` 不遵循透明布局，使其确实内部可变 —— 则 `&T` 同 `PartialMut<T>`
- 完全遵循透明布局，`Cell` 本质上就是整体可变 —— 则 `&T` 同 `&mut T`（仅当 `T` 透明至
  `UnsafeCell`），其它的 `&T` 仍同 `PartialMut<T>`

两者都不够干净，暂时没有结论
