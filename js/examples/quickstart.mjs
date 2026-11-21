// A tour of the zindi client.
//
//   npm install zindi
//   ZINDI_USERNAME=you ZINDI_PASSWORD=... node examples/quickstart.mjs
//
// Without credentials the script only browses public data (competitions, leaderboard).
// Inside this repository, run `npm run build` first.

import { AuthenticationError, Client } from "zindi";

const zindi = new Client();

// 1. Browse open competitions (no login needed).
const open = await zindi.competitions({ status: "open", limit: 5 });
for (const comp of open) {
  const ends = comp.endTime ? comp.endTime.toISOString().slice(0, 10) : "no end date";
  console.log(`${comp.id.padEnd(45)} ${String(comp.reward ?? "").padEnd(20)} ends ${ends}`);
}
if (open.length === 0) process.exit(0);

// 2. Look at one of them, and select it as the default competition.
const comp = await zindi.competition(open[0].id);
await zindi.use(comp.id);
console.log(`\n${comp.title}: metric ${comp.metric ?? "?"}, ${comp.participants ?? 0} participants`);
for (const file of comp.dataFiles) console.log(`  ${file.filename} (${file.sizeHuman})`);

// 3. The top of the leaderboard (no login needed either).
for (const row of await zindi.leaderboard(undefined, { limit: 3 })) {
  console.log(`  #${row.rank} ${row.name} ${row.score ?? ""}`);
}

// 4. Everything else needs credentials.
if (!(await zindi.isAuthenticated())) {
  console.log("\nSet ZINDI_TOKEN, or ZINDI_USERNAME and ZINDI_PASSWORD, to join and submit.");
  process.exit(0);
}

try {
  const me = await zindi.me();
  console.log(`\nLogged in as ${me.username}`);
  const joined = await zindi.join(); // false if you were already in
  console.log(joined ? "Joined the competition." : "Already a participant.");
  const paths = await zindi.download(undefined, "data");
  console.log(`Data files: ${paths.join(", ")}`);
  const limits = await zindi.limits();
  console.log(`Submissions left today: ${limits.remainingToday ?? "?"}`);
  // const sub = await zindi.submit("submission.csv", { comment: "baseline", wait: true });
  // console.log(`Scored ${sub.score}`);
} catch (error) {
  if (error instanceof AuthenticationError) console.error(error.message);
  else throw error;
}
