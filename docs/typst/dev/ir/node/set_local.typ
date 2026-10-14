#import "/lib/lib.typ": *;
#show: template;

#let title = [SetLocal];
#let tag = <tag-set-local>;

#import "/lib/graph.typ";

#{
    import graph: *;
    node({
        "SetLocal<T>"
        ctrl_in()
        ctrl_out()
        data_in("LocalRef")[setter]
        data_in("T")[value]
    })
}

将#link(<tag-local>)[Local]设为指定值

