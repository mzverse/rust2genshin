#import "/lib/lib.typ": *;
#show: template;

#let title = [unreachable];
#let tag = <tag-unreachable>;
#set raw(lang: "rust");

#import "/lib/graph.typ";

#{
    import graph: *;
    node({
        [unreachable]
        ctrl_in()
    })
}

不可达的节点，对应`unreachable!()`等，表示一个不可达的分支

目前来看，其无需提供#link(<tag-never>)[`Never`]或出控制流

如果一个节点必然到达unreachable，则其也是不可达的

lower时会直接消除，但优化时就应已消除
