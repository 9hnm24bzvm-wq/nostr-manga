// A single actual title gesture is held before native forwarding. Finishing
// its control interaction before fault activation makes the finite checkpoints
// ordered, without manufacturing a denial, success, capacity receipt or retry.
export async function runHeldAccountDenial({ hold, pattern, match, gesture, capacity, denial, check }) {
  const pressure = await hold(pattern, match, true);
  try {
    await gesture();
    await check("pressure-captured", () => pressure.wait());
    await capacity(async () => {
      // The genuine capacity helper invokes this callback only after its own
      // native429/capacity_busy prerequisite has passed.
      await check("capacity-ready", async () => {});
      await check("pressure-released", () => pressure.release());
      await denial();
    });
  } finally { await pressure.close(); }
}
