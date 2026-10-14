// Fixed-ID diagnostic wrapper; it cannot consume or change an assertion failure.
export function createJourneyCheck(report, save = () => {}) {
  report.journey_checks = [];
  return async (check_id, action) => {
    const row = { check_id, status: 'running' };
    report.journey_checks.push(row); save();
    try { const result = await action(); row.status = 'passed'; return result; }
    catch (error) { row.status = 'failed'; throw error; }
    finally { save(); }
  };
}
