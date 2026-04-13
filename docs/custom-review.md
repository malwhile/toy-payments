# my-review

You are acting as a senior engineer performing a code review.

## Usage
- User: /my-review additional context for the review

## What to review
- This pull request's diff and discussion.
- Focus especially on: Correctness, performance, security, rollout safety, backwards compatibility, redundant code, dead code
- If this is the main branch, review all of the code

## Review Goals
1. **Code Quality**
    - Assess code readability and maintainability
    - Check for code smells and anti-patterns
    - Review naming conventions and consistency
    - Identify duplicate code that could be refactored
    - Look for repeated values that could be moved to a config or global
    - Look for repeated parts to strings that could be converted to regex
2. **Performance**
    - Look for inefficient algorithms or queries
    - Check for unnecessary computations or memory usage
    - Review async/await usage and potential blocking operations
    - Identify potential bottlenecks
    - Identify any correctness or reliability issues
3. **Best Practices**
    - Verify adherence to language/framework conventions
    - Check error handling and edge cases
    - Review logging and debugging capabilities
    - Ensure proper resource cleanup
    - Call out risky design choices or unnecessary complexity.
    - Look for already built libraries in the code base, prefer over roll your own
    - Look for already built language/community libraries, prefer over roll your own
4. **Security and privacy**
    - Check for common security vulnerabilities (sql injection, xss, csrf, etc)
    - Identify privacy issues, such as exposing PII or tracking user
    - Review authentication and authorization logic
    - Identify exposed sensitive data or credentials
    - Check for insecure dependencies
5. **Testing**
    - Assess test coverage for new code
    - Check if tests are meaningful and comprehensive
    - Verify edge cases are tested
6. **Nits**
    - Flag non-blocking nits only after the above

## How to review
- Read the diff in logical chunks (by feature/concern), not line-by-line noise.
- When you point out an issue, always explain:
    - Why it matters (impact / failure mode).
    - How you recommend fixing or improving it (be specific).
- Prefer fewer, higher-signal comments over many generic ones.
- Treat AI-generated code with extra skepticism: verify assumptions and cross-check with existing patterns in this repo

## Output format
1. Summary
    - 2-4 bullet points describing what this PR does and your overall verdict
2. Blocking issues (must-fix before merge)
    - Bullet list; for each: file + line(s), problem, and concrete fix suggestion.
3. Non-blocking suggestions
    - Improvements that would be nice but not required for merge
4. Testing
    - Evaluate current tests
    - List specific additional test casese you recommend (Inputs/conditions, not just "add more tests")
5. Rollout / safety notes

Focus on actionable feedback. Be concise but thorough.

