# JavaScript / TypeScript

```bash
npm install zindi
```

Node.js 18 or newer, no dependencies, ES modules, full TypeScript types.

```ts
import { Client } from "zindi";

const zindi = new Client();                     // ZINDI_TOKEN, or the file saved by `zindi login`
const open = await zindi.competitions({ status: "open", limit: 10 });
await zindi.join("Crop-Yield-Forecast-Challenge");
await zindi.download("Crop-Yield-Forecast-Challenge", "data");
const sub = await zindi.submit("submission.csv", {
  competition: "Crop-Yield-Forecast-Challenge",
  comment: "first try",
  wait: true,
});
console.log(sub.publicScore);
```

The full method list and options are in the
[package README](https://github.com/eaedk/zindi-package/tree/main/js#readme).
