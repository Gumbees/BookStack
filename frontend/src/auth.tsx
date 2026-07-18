import { createContext, createSignal, useContext, type ParentProps } from 'solid-js';
import { api, clearSession, getStoredUser, getToken, storeSession } from './api';
import type { User } from './types';

interface AuthContextValue {
  user: () => User | null;
  loggedIn: () => boolean;
  canEdit: () => boolean;
  isAdmin: () => boolean;
  login: (email: string, password: string) => Promise<void>;
  logout: () => void;
}

const AuthContext = createContext<AuthContextValue>();

export function AuthProvider(props: ParentProps) {
  const [user, setUser] = createSignal<User | null>(getStoredUser());

  const value: AuthContextValue = {
    user,
    loggedIn: () => !!getToken() && !!user(),
    canEdit: () => {
      const u = user();
      return !!u && (u.role === 'admin' || u.role === 'editor');
    },
    isAdmin: () => user()?.role === 'admin',
    login: async (email, password) => {
      const res = await api.post<{ token: string; user: User }>('/auth/login', { email, password });
      storeSession(res.token, res.user);
      setUser(res.user);
    },
    logout: () => {
      clearSession();
      setUser(null);
    },
  };

  return <AuthContext.Provider value={value}>{props.children}</AuthContext.Provider>;
}

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth outside AuthProvider');
  return ctx;
}
