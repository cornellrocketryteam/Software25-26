import psycopg2
import sys

# ==============================================================================
# DATABASE CONFIGURATION
# ==============================================================================
DB_HOST = "192.168.8.193"
DB_PORT = "5432"
DB_NAME = "postgres"
DB_USER = "postgres"
DB_PASS = "Rocketry2526"

def wipe_database():
    print("\n" + "="*50)
    print("WARNING: PRE-FLIGHT DATABASE RESET")
    print("="*50)
    print("This will PERMANENTLY delete all telemetry data from:")
    print(f" -> Database: {DB_NAME} at {DB_HOST}")
    print(f" -> Table: telemetry_data")
    print("="*50 + "\n")

    confirm = input("Type 'NUKE' to confirm deletion of all data: ")
    
    if confirm != "NUKE":
        print("Abort. Database was NOT modified.")
        sys.exit(0)

    try:
        conn = psycopg2.connect(
            host=DB_HOST,
            port=DB_PORT,
            dbname=DB_NAME,
            user=DB_USER,
            password=DB_PASS
        )

        conn.autocommit = True
        cursor = conn.cursor()

        print("\nWiping telemetry_data table...")
        cursor.execute("TRUNCATE TABLE telemetry_data;")
        
        print(" Database successfully reset! Ready for flight.")

    except Exception as e:
        print(f"\n ERROR connecting to database or executing command:\n{e}")
    finally:
        if 'cursor' in locals():
            cursor.close()
        if 'conn' in locals():
            conn.close()

if __name__ == "__main__":
    wipe_database()
