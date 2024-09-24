import { run } from "../index.js";

console.log(
  run((v) => {
    console.log("call with", v);
    return (parseInt(v) + 1).toString();
  }),
);
