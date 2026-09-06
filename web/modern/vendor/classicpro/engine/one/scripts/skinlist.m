/********************************************************\
**  Filename:	skinlist.m				**
**  Version:	1.0					**
**  Date:	27. Jan. 2008 - 20:19 			**
**********************************************************
**  Type:	winamp.wasabi/maki			**
**  Project:	cPro engine				**
**********************************************************
**  Author:	Martin Poehlmann aka Deimos		**
**  E-Mail:	martin@skinconsortium.com		**
**  Internet:	http://www.skinconsortium.com		**
**		http://home.cs.tum.edu/~poehlman	**
\********************************************************/


#include <lib/std.mi>

Function deleteSkinPos(int itemnum);

Global GuiList skinlist;
Global Button skinswitch;
Global Popupmenu myMenu;
Global Boolean msgbox_open = false;

System.onScriptLoaded ()
{
	skinlist = getscriptGroup().findObject("skinlist");
	skinswitch = getscriptGroup().findObject("switch");

	int n = getPublicInt("classicPro_skinlist_numskins", 0);

	boolean isPartOf = false;

	for ( int i = 0; i < n; i++ )
	{	
		String name = getPublicString("classicPro_skinlist_nr"+integertoString(i), "");
		String type = getPublicString("classicPro_skinlist_tnr"+integertoString(i), "");
		skinlist.addItem(name);
		skinlist.setSubItem(i,1, translate(type));
		if (name == getSkinName())
			isPartOf = true;		
	}

	if (!isPartOf)
	{
		String type = strleft(strRight(getParam(),5),4);
		if (type == ".tmp")
		{
			type = "Zipped";
		}
		else
		{
			type = "Folder";
		}
		
		setPublicString("classicPro_skinlist_nr"+integertoString(i), getSkinName());
		setPublicString("classicPro_skinlist_tnr"+integertoString(i), type);
		setPublicInt("classicPro_skinlist_numskins", n+1);
		skinlist.addItem(getSkinName());
		skinlist.setSubItem(n,1, translate(type));
	}
}

skinlist.onDoubleClick(int itemnum)
{
	String skin = skinlist.getItemLabel(itemnum, 0);

	if (getItemLabel(itemnum, 1) == translate("Zipped"))
	{
		skin += ".wal";
	}
	
	switchskin(skin);
}

skinlist.onRightClick(int itemnum)
{
	myMenu = new PopupMenu;
	myMenu.addCommand("Switch to this skin", 1, 0, 0);
	myMenu.addCommand("Remove from list", 4, 0, 0);
	
	int a = myMenu.popAtMouse();
	delete myMenu;
	
	//int a = messageBox("Do you want to remove this Skin from the list?", "", 4, "");

	if (a == 4)
	{
		deleteSkinPos(itemnum);
	
	}
	else if(a==1){
		skinlist.onDoubleClick(itemnum);
	}
}

deleteSkinPos(int itemnum){
		skinlist.deleteByPos(itemnum);

		int n = skinlist.getNumItems();
		setPublicInt("classicPro_skinlist_numskins", n);

		for ( int i = 0; i < n; i++ )
		{
			setPublicString("classicPro_skinlist_nr"+integertoString(i), skinlist.getItemLabel(i, 0));
			setPublicString("classicPro_skinlist_tnr"+integertoString(i), skinlist.getItemLabel(i, 1));		
		}
}


skinswitch.onLeftClick ()
{
	myMenu = new PopupMenu;
	myMenu.addCommand("Switch to selected skin", 1, 0, 0);
	myMenu.addCommand("Remove selected skin from list", 2, 0, 0);
	myMenu.addSeparator();
	myMenu.addCommand("Clear all skins from list", 3, 0, 0);
	myMenu.addCommand("Get more ClassicPro skins", 4, 0, 0);

	int a = myMenu.popAtXY(clientToScreenX(skinswitch.getLeft()), clientToScreenY(skinswitch.getTop() + skinswitch.getHeight()));
	delete myMenu;

	if(a==1){
		skinlist.onDoubleClick(skinlist.getItemFocused());
	}
	else if(a==2){
		deleteSkinPos(skinlist.getItemFocused());
	}
	else if(a==3){
		if(msgbox_open) return;
		msgbox_open = true;
		int b = messageBox("Are you sure you want to remove all skins from the list?\n\nTo add a ClassicPro skin to this list again, just load the skin again.", "ClassicPro", 4, "");
		msgbox_open = false;

		if(b==4){
			int n = skinlist.getNumItems();
			
			for ( int i = 0; i < n; i++ )
			{
				deleteSkinPos(0);
			}
		}
	}
	else if(a==4){
		System.navigateUrl("http://www.skinconsortium.com/index.php?page=Downloads&typeID=4");
		//add opentab to sui system.. then goto browser.... TODO
	}
}
