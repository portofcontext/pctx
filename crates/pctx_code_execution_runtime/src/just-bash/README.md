Current `just-bash` version: `2.6.0`

1. deno bundle --platform browser --minify --external "@mongodb-js/zstd" --external "node-liblzma" entry.ts -o bundle.js

2. replace node:zlib imports with local stubs

```bash
sed -i '' 's|from"node:zlib"|from"./node_zlib_stub.js"|g' bundle.js && grep -c "node:zlib" bundle.js || echo "0 instances remaining"
```
