/** Runtime-validated records and durable browser protocol contracts. */
export type Family = "chapters" | "series" | "lists" | "members";
export type ReadingStatus =
  | "reading"
  | "on_hold"
  | "plan_to_read"
  | "dropped"
  | "re_reading"
  | "completed";
export type ChapterRecord = {
  id: string;
  completed: boolean;
  page: number | null;
  updated_at: number;
};
export type SeriesRecord = {
  id: string;
  status: ReadingStatus;
  updated_at: number;
};
export type ListRecord = { id: string; name: string; updated_at: number };
export type MemberRecord = {
  list: string;
  series: string;
  position: number;
  updated_at: number;
};
export type FamilyRecord = {
  chapters: ChapterRecord;
  series: SeriesRecord;
  lists: ListRecord;
  members: MemberRecord;
};
export type LibraryRecord = FamilyRecord[Family];
export type Records = { [K in Family]: LibraryRecord[] };
export type RecordRow = { family: Family; key: string; record: LibraryRecord };
export type Statistics = Record<Family, number> & { total: number };
export type Counts = Statistics & {
  revision: number;
  recordLimit: number;
  overBudget: boolean;
};
export type LibrarySnapshot = {
  format: "manga-local-library";
  version: 1;
  revision?: number;
  exported_at?: string;
  records: Records;
};
export type LocalSnapshot = LibrarySnapshot & {
  revision: number;
  exported_at: string;
};
export type GraphObject = {
  type: "root" | "chunk";
  hash: string;
  data: Uint8Array<ArrayBuffer>;
};
export type Graph = {
  root: GraphObject;
  objects: GraphObject[];
  records: number;
};
export type PreparedSnapshot = { revision: number; graph: Graph };
export type Descriptor = {
  hash: string;
  size: number;
  records: number;
  family: Family;
  buckets: number;
  index: number;
};
export type EventTemplate = {
  pubkey?: string;
  created_at: number;
  kind: number;
  content: string;
  tags: string[][];
};
export type SignedEvent = EventTemplate & {
  pubkey: string;
  id: string;
  sig: string;
};
export type Signer = { signEvent(event: unknown): Promise<unknown> };
export type Relay = { origin: string; serviceKey: string; selected?: boolean };
export type Recipient = Relay & {
  cursor: number;
  state: string;
  message: string;
  projected?: boolean;
};
export type PublicationClock = {
  owner: string;
  id: string;
  created_at: number;
};
export type Job = {
  id: string;
  event: SignedEvent;
  revision: number;
  relayRevision?: number;
  createdAt: number;
  deviceRevision?: number;
  sourceScope?: {
    origin: string;
    serviceKey: string | null;
    catalogNamespace: string | null;
  };
  cancelled: boolean;
  objectCount: number;
  recipients: Recipient[];
};
export type NewJob = Omit<Job, "objectCount"> & {
  objectCount?: number;
  objects: GraphObject[];
};
export type StoredJob = Job & { objects?: GraphObject[] };
export type JobObjectRow = { job: string; index: number; object: GraphObject };
export type JobBatch = { objects: GraphObject[]; next: number };
export type JsonObject = { [key: string]: unknown };
