export type UnlistenFn = () => void;

export function getCurrentWebview() {
  return {
    async onDragDropEvent(
      handler: (event: { payload: { type: string; paths: string[] } }) => void,
    ): Promise<UnlistenFn> {
      void handler;
      return () => undefined;
    },
  };
}
