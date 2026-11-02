# Repository Instructions

## CRITICAL — DEPLOYMENT CONFIGURATION IS PROTECTED

1. AI coding agents MUST NOT modify `.github/workflows/deploy.yml` in future tasks.
2. This prohibition applies to direct and indirect changes.
3. Do not change the Docker Swarm service name `smpalfa_exam`.
4. Do not change the Docker image `taqiussa/smpalfa:exam`.
5. Do not add an `@sha256` digest to the image reference.
6. Do not remove `--no-resolve-image`.
7. Do not remove `--force`.
8. Do not change the established deployment timing configuration, including `--update-delay 10s`.
9. Do not refactor the deployment workflow.
10. Do not modify deployment for optimization, cleanup, security improvement, or standardization.
11. A change is allowed only when the user explicitly requests a change to `.github/workflows/deploy.yml`.
12. This instruction has the highest priority among internal repository coding rules unless it conflicts with a higher-level system instruction.
