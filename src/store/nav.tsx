import { createContext, useContext } from "react";

export interface Nav {
  active: string;
  navigate: (id: string) => void;
}

export const NavContext = createContext<Nav>({ active: "dashboard", navigate: () => {} });

export function useNav(): Nav {
  return useContext(NavContext);
}
