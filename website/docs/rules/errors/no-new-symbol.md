<!--
 generated docs file, do not edit by hand, see xtask/docgen 
-->
# no-new-symbol

Disallow constructing `Symbol` using `new`.

`Symbol` shouldn't be constructed using `new` keyword since it results in a `TypeError`, instead
it should be called as a function.

## Incorrect code examples

```js
// This call results in TypeError
const fooSymbol = new Symbol("foo");
```

## Correct code examples

```js
const fooSymbol = Symbol("foo");
```

::: details More incorrect examples

```js
new Symbol()
```
:::
::: details More correct examples

```js
Symbol()
```

```js
new SomeClass()
```
:::

[Source](https://github.com/rslint/rslint/tree/master/crates/rslint_core/src/groups/errors/no_new_symbol.rs)