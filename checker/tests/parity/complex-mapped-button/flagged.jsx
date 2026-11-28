const List = ({ items, active, select }) => items.map((item) => (
  <button onClick={() => select(item)} className={item === active ? 'on' : 'off'} aria-pressed={item === active}>
    {item}
  </button>
));
export { List };
