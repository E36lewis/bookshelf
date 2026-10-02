using Bookshelf.Services;
using Microsoft.UI.Xaml.Controls;

namespace Bookshelf.Pages;

/// <summary>What every page of the app can reach: the window and the open journal.</summary>
public class BookshelfPage : Page
{
    /// <summary>The app's window.</summary>
    internal static MainWindow Shell => MainWindow.Current;

    /// <summary>The open journal and profile.</summary>
    internal static Session Session => MainWindow.Current.Session;
}
