#import "template.typ": *;
#import "style.typ": *;

#import "fp.typ": *;
// #import "object.typ": *;
#import "class.typ": *;
#import "iterator.typ": *;

#import "mth.typ";

/// use in bundle main.typ
#let bundle_document(name) = {
    import name + ".typ" as p;
    document(name + ".html", title: dictionary(p).at("title", default: none))[#p]
}
/// use in bundle main.typ
#let bundle_asset(name) = {
    asset(name, read(name, encoding: none))
}

#let custom_element(register, ele) = {
    [#metadata(register) <custom-element>]
    ele
}
