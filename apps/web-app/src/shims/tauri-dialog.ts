export type DialogOptions = {
  multiple?: boolean;
  directory?: boolean;
  title?: string;
  filters?: { name: string; extensions: string[] }[];
  defaultPath?: string;
};

export async function open(options?: DialogOptions): Promise<string | string[] | null> {
  void options;
  return null;
}

export async function save(options?: DialogOptions): Promise<string | null> {
  void options;
  return null;
}
