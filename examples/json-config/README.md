# Cloud Infrastructure Config Versioning

Version control your cloud infrastructure configs without merge conflicts.

## Problem

Teams managing cloud infrastructure (Terraform, CloudFormation, custom configs) often
step on each other's toes. One engineer adds a subnet, another updates a security
group, and the merge becomes a text-level conflict nightmare — even though the
changes are structurally independent.

## Solution

Suture understands JSON semantically. When two teams modify different keys in the
same config file, Suture merges them automatically without conflicts. No more
hand-resolving JSON brackets and braces.

## What This Example Demonstrates

- Committing a base cloud infrastructure config (VPC, subnets, security groups)
- Two parallel feature branches modifying the same JSON file
- Semantic merge combining both changes without conflicts
- Using `suture diff` to inspect what changed between branches

## Running

```bash
make
```

## Expected Outcome

Both `feature-a` (new subnet) and `feature-b` (updated security group rules) merge
cleanly into `main`. The final config contains changes from both branches.
