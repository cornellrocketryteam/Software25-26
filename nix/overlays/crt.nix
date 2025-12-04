_: prev: {
  crt = prev.crt.overrideScope (
    _: _: {
      crt-software-root = ../..;
    }
  );
}
