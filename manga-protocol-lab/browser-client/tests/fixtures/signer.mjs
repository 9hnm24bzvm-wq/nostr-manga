import { createHash } from "node:crypto";
import { schnorr } from "@noble/curves/secp256k1.js";
export function createSigner(number = 1) {
  const secret = Buffer.from(number.toString(16).padStart(64, "0"), "hex");
  const pub = Buffer.from(schnorr.getPublicKey(secret)).toString("hex");
  return {
    pub,
    getPublicKey: async () => pub,
    async signEvent(template) {
      const event = { ...template, pubkey: pub };
      event.id = createHash("sha256")
        .update(
          JSON.stringify([
            0,
            pub,
            event.created_at,
            event.kind,
            event.tags,
            event.content,
          ]),
        )
        .digest("hex");
      event.sig = Buffer.from(
        schnorr.sign(Buffer.from(event.id, "hex"), secret),
      ).toString("hex");
      return event;
    },
  };
}
