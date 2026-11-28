const run = (a: boolean, b: boolean, c: boolean): number => {
  if (a) {
    if (b) {
      if (c) {
        return 1;
      }
    }
  } else if (b) {
    if (c) return 2;
  }
  return 0;
};
export { run };
