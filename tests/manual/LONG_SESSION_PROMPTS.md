# 60 follow-ups for the long Codex session (S1 of LONG_SESSION_PLAN.md)

Paste them one at a time, in order, into the same Codex session after the first prompt. Prompts 1, 20, 40 and 60 ask for the project rules: write down each answer. Prompts 48-52 and 59 exercise decisions and the brief. Stop after 60; do not start pricing.

1. List the project rules.
2. Show me the current file layout and say what each file is for.
3. Print the SQLite schema and explain every table and column.
4. Why did you choose normalized tables instead of one wide invoices table? Give two alternatives you rejected.
5. Add a unit test that creates 3 invoices in the same year and checks the numbers are 0001, 0002, 0003.
6. Add a unit test that the counter restarts at 0001 in a new year.
7. What happens if two processes create an invoice at the same moment? Walk through the exact SQL.
8. Write a test that starts two processes writing invoices at once and checks no number repeats.
9. Add an index you think is missing, and say why.
10. Explain how a line item's total is stored and why it is an integer.
11. What validation does the Invoice dataclass do? List every check.
12. Add a test for a negative quantity.
13. Add a test for a zero-price line.
14. What are the three most likely bugs in the code so far? Be specific, with line references.
15. Fix the one you think is most likely, with a test.
16. Show how a customer is stored and how a second invoice reuses the same customer.
17. Add a test for the same customer on two invoices.
18. Summarise in 10 lines everything decided so far and why.
19. What would break if we later added a currency field? Don't add it.
20. List the project rules.
21. Review storage.py as if you were a hostile reviewer. List problems.
22. Fix the two most serious ones.
23. Explain how a database error in the middle of creating an invoice is handled.
24. Write a test that forces a failure after the invoice row is written and checks nothing is left behind.
25. How does the code read an invoice back? Show the query and the object it returns.
26. Add a function to list invoices for a customer, newest first, with a test.
27. Add a function to fetch one invoice by its number, with a test.
28. What should happen when the number does not exist? Implement and test that.
29. Show the whole numbering code and explain it line by line.
30. Is there any place a date could be stored in a non-ISO format? Check every code path.
31. Add a test that rejects a date written 05.03.2026 when storing.
32. Explain why printed dates will differ from stored dates. Where will that conversion live?
33. Write that conversion function (ISO to DD.MM.YYYY) and its test. Don't use it anywhere yet.
34. Review the tests directory: what is not covered?
35. Add the missing tests you think matter most.
36. Run the whole test suite the way the project says it must be run, and show the output.
37. How long does the suite take? Is anything slow?
38. Write a short README section describing models, storage and numbering.
39. What are the open questions for the pricing step? List them, don't answer yet.
40. List the project rules.
41. Which rules will pricing have to respect most carefully, and why?
42. Sketch (in words only, no code) the data model pricing will need.
43. What did we decide about tax so far? Quote the rule.
44. Explain in two sentences why rounding half up matters for tax.
45. Add a test helper that builds an invoice with N lines so later tests are shorter.
46. Refactor two tests to use the helper.
47. What would you change about the schema if there were 10 million invoices?
48. Don't change it. Write that down as a note for later.
49. Show me everything you have recorded with decision.record so far, and what is missing.
50. Record any decision you made but haven't recorded.
51. Show me the brief as you would write it now.
52. Update the brief: goal, where it stands, ruled out, next steps.
53. What is the single riskiest thing about handing this to another agent tomorrow?
54. What should the next agent read first?
55. Is there anything in the repo that would confuse someone new? Fix the worst.
56. Check that nothing in the code prints or logs a customer email.
57. Check that nothing in the code uses a float anywhere.
58. Check that nothing imports anything outside the standard library.
59. Give me a final status: done, not done, risks, next steps.
60. List the project rules.
