#import "/lib/lib.typ": *;
#show: template;

#let title = [switch];
#set raw(lang: "rust");

#import "/lib/graph.typ";

#{
    import graph: *;
    node({
        [switch\<[T; N]\>(#link(<tag-switch-values>)[values: [T; N]])]
        ctrl_in()
        ctrl_out[#link(<tag-switch-cases>)[..cases[N]]]
        ctrl_out[#link(<tag-switch-default>)[default]]
        data_in("T")[#link(<tag-switch-discr>)[discr]]
    })
}

分支节点，源于`if`或`match`等

控制流顺序与MIR中的`TerminatorKind::SwitchInt`一致

支持`bool`和`i32`

lower后得到if或switch节点

优化时应去除连接到#link(<tag-unreachable>)[unreachable]的分支

= discr <tag-switch-discr>

判别式，决定了走哪个分支

若等于#link(<tag-switch-values>)[`values[i]`]则走#link(<tag-switch-cases>)[`cases[i]`]，均不匹配则走#link(<tag-switch-default>)[default]

= values <tag-switch-values>

一些常量，原则上不应有重复

每个值对应一个分支

= cases <tag-switch-cases>

每个值对应的分支

= default <tag-switch-default>

当没有任何值被匹配时走此分支
