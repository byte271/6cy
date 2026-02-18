# Sixcy Security Profile

This document outlines the security design, responsible disclosure process, and vulnerability reporting channels for the Sixcy project.

## 1. Security Design Principles

Sixcy is designed with a focus on data integrity and safe parsing:

*   **Memory Safety**: The reference implementation is written in Rust to prevent common memory-related vulnerabilities such as buffer overflows and use-after-free.
*   **Strict Validation**: All binary headers, including the Superblock and Data Block headers, are strictly validated for magic numbers, version compatibility, and field consistency.
*   **Resource Limits**: The parser enforces limits on block sizes and metadata lengths to mitigate resource exhaustion attacks (e.g., compression bombs).
*   **Two-Layer Integrity Checks**: Each block carries both a CRC32 checksum of the compressed payload (fast corruption detection) and a BLAKE3 hash of the uncompressed content (cryptographic integrity). Both are verified on every read.
*   **Merkle Root Verification**: The FileIndex includes a BLAKE3 Merkle root over all block content hashes, enabling out-of-band verification of the complete archive without reading block payloads.

## 2. Threat Model (Public Summary)

Sixcy addresses the following high-level threats:

*   **Malformed Input**: Handling of crafted archives designed to exploit parser logic.
*   **Resource Exhaustion**: Mitigation of attacks that attempt to consume excessive CPU or memory through malicious compression ratios.
*   **Data Corruption**: Detection of accidental or intentional bit-flips in the stored data via CRC32 (compressed) and BLAKE3 (uncompressed) verification.
*   **Content Substitution**: The BLAKE3 `Content Hash` field in each block header makes it computationally infeasible to substitute block payloads without detection, even if CRC32 is bypassed.

## 3. Responsible Disclosure

We take the security of our users seriously. If you find a security vulnerability, we appreciate your help in disclosing it to us in a responsible manner.

### 3.1 Reporting a Vulnerability

Please do not report security vulnerabilities via public GitHub issues. Instead, send a detailed report to our security team.

**Reporting Channel**: [Insert Security Email or Link to Disclosure Platform]

Your report should include:
*   A description of the vulnerability.
*   Steps to reproduce the issue (including a proof-of-concept if possible).
*   Potential impact of the vulnerability.

### 3.2 Response Process

1.  **Acknowledgment**: We will acknowledge receipt of your report within 48 hours.
2.  **Investigation**: Our team will investigate the issue and determine its severity.
3.  **Fix**: We will work on a patch to address the vulnerability.
4.  **Disclosure**: Once a fix is available and users have had time to update, we will publish a security advisory.

## 4. Security Updates

Security advisories and updates will be announced through the project's official release channels. We recommend all users keep their Sixcy implementation updated to the latest stable version.
