# swc-plugin-static-jsx

[![Rust](https://github.com/Desdaemon/swc-plugin-static-jsx/actions/workflows/rust.yml/badge.svg)](https://github.com/Desdaemon/swc-plugin-static-jsx/actions/workflows/rust.yml)
[![Node.js E2E](https://github.com/Desdaemon/swc-plugin-static-jsx/actions/workflows/e2e.yml/badge.svg)](https://github.com/Desdaemon/swc-plugin-static-jsx/actions/workflows/e2e.yml)

SWC plugin to transform JSX calls to static templates

## Install

> **Note**
> This plugin is not yet published on npmjs.com. Until such time, you'll need to compile the plugin from source to use it.

```shell
npm i -D @swc/core swc-plugin-static-jsx
```

### From source

```shell
git clone https://github.com/Desdaemon/swc-plugin-static-jsx
cd swc-plugin-static-jsx
rustup target add wasm32-wasi
npm run dist
npm link
```

## Usage

```jsonc
// In .swcrc:
{
  "jsc": {
    "experimental": {
      "plugins": [
        // All config values are optional.
        [
          "swc-plugin-static-jsx",
          {
            // If an identifier is supplied, it should not be an ambient global. Can be null.
            "template": "String.raw",
            // If supplied, template will be imported as `import { template } from 'my-library'`
            "importSource": "my-library",
            "spread": "$$spread",
            "child": "$$child",
            "children": "$$children"
          }
        ]
      ]
    }
  }
}
```

In your tsconfig.json, `compilerOptions.jsx` should be set to 'preserve'. You will also need to
provide your own JSX-related types under `namespace JSX`.

## Sample

```jsx
let unsanitized = '<script>alert("You\'ve been pwned!")</script>';
// input
function MyComponent() {
  return (
    <div foo="bar" baz={true} {...spread} {...{ "std::string": "value" }}>
      The quick brown fox jumps over the <strong>lazy</strong> dog.
      {unsanitized}
      {...children}
    </div>
  );
}

// output (approximate)
function MyComponent() {
  return html`<div foo="bar" baz ${{ $$spread: spread }} std::string="value">
    The quick brown fox jumps over the<strong>lazy</strong>dog. ${{
      $$child: unsanitized,
    }} ${{ $$children: children }}
  </div>`;
}
```

Sample implementation of `html`:

```tsx
function html(raw, ...children: Record<string, unknown>[]) {
  all: for (const child of children) {
    if ("$$spread" in child) {
      // ..
      continue all;
    }
    if ("$$child" in child) {
      // ..
      continue all;
    }
    if ("$$children" in child) {
      // ..
      continue all;
    }
    // ..
  }
}
```
