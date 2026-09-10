'use client';

// La app arranca en el explorador del NAS: conectar por SMB y navegar por las
// carpetas es el punto de entrada. La biblioteca y el lector de Readest siguen
// ahí (/library y /reader): al abrir un libro del NAS se importa y se salta a
// su lector, sin tocarlo.
import NasPage from './nas/page';

export default function HomePage() {
  return <NasPage />;
}
