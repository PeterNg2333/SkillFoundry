const grade = (a: boolean, b: boolean) => {
  if (a && b) return 1;
  if (a || b) return 2;
  return 3;
};
const simple = (a: boolean) => (a ? 1 : 2);
export { grade, simple };
