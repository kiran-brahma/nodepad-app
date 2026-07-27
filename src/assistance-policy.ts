import type { AssistancePolicy } from "./workspace-client"

/** The Assistance Policies a Thinking Workspace may be in, in the order
 *  every surface offers them. One list, so a policy cannot exist as a button
 *  the palette has never heard of. */
export const ASSISTANCE_POLICIES = ["manual", "local_ai", "cloud_ai"] as const

const POLICY_LABELS: Record<AssistancePolicy, string> = {
  manual: "Manual",
  local_ai: "Local AI",
  cloud_ai: "Cloud AI",
}

/** An Assistance Policy reads as words, not as the stored identifier. */
export function assistancePolicyLabel(policy: AssistancePolicy): string {
  return POLICY_LABELS[policy]
}
