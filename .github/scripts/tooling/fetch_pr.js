const { Octokit } = require("@octokit/rest");
const fs = require("fs");

const octokit = new Octokit({
  auth: process.env.GITHUB_TOKEN,
});

async function fetchClosedPRs() {
  const owner = process.env.GITHUB_REPOSITORY_OWNER;
  const repo = process.env.GITHUB_REPOSITORY.split("/")[1];

  const { data: pulls } = await octokit.pulls.list({
    owner,
    repo,
    state: "closed",
    per_page: 100,
  });

  const filteredPRs = pulls.filter((pr) =>
    pr.head.ref.includes("tooling-wallet")
  );

  const changelog = filteredPRs
    .map((pr) => `- ${pr.title} (#${pr.number}) by @${pr.user.login}`)
    .join("\n");

  fs.writeFileSync("changelog.txt", changelog);
}

fetchClosedPRs().catch((error) => {
  console.error(error);
  process.exit(1);
});